#!/usr/bin/env python3
"""
VibeLoop - Osu! x Hush 2 Haptic Integration
============================================
Reads real-time game state from tosu (WebSocket)
and maps hit accuracy / combo / health to Hush 2 vibration
via Intiface Central (buttplug-py).

Requirements:
    pip install buttplug websockets

Usage:
    1. Start Intiface Central and click "Start Server"
    2. Connect your Hush 2 in Intiface Central
    3. Start tosu
    4. Start osu!lazer
    5. Run: python3 vibeloop_osu.py
"""

import asyncio
import json
import time
import logging
import sys
import os
from dataclasses import dataclass

# Allow importing vibeloop_host from the parent directory
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from buttplug import ButtplugClient, DeviceOutputCommand, OutputType

# ─── Logging ────────────────────────────────────────────────────────────────
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    handlers=[logging.StreamHandler(sys.stdout)],
)
log = logging.getLogger("VibeLoop")

# ─── Configuration ──────────────────────────────────────────────────────────

INTIFACE_WS  = "ws://localhost:12345"
UPDATE_INTERVAL = 0.01   # seconds between vibration updates

# ─── Intensity Levels ────────────────────────────────────────────────────────

IDLE        = 0.0   # not in a map
MEH         = 0.07  # 50  — Meh
OK          = 0.15  # 100 — OK
GREAT       = 0.30  # 300 — Great
PERFECT     = 0.50  # 300 with geki — Perfect
MISS        = 0.75  # miss
FAIL        = 1.0   # health depleted / failed

# How long each event pulse lasts (seconds)
MEH_DURATION     = 0.07
OK_DURATION      = 0.09
GREAT_DURATION   = 0.11
PERFECT_DURATION = 0.14
MISS_DURATION    = 0.2

# Smoothing: how fast intensity fades back down after a pulse (0.0–1.0 per tick)
SMOOTH_DOWN = 0.2

# Win pattern duration in seconds
WIN_DURATION = 4.0


# ─── Game State ──────────────────────────────────────────────────────────────

@dataclass
class GameState:
    status:   int   = 0
    health:   float = 0.0
    hit300:   int   = 0
    hit100:   int   = 0
    hit50:    int   = 0
    hit_miss: int   = 0
    geki:     int   = 0
    katu:     int   = 0
    accuracy: float = 0.0
    failed:   bool  = False


# ─── Haptic Engine ───────────────────────────────────────────────────────────

import random

class HapticEngine:
    def __init__(self):
        self.state = GameState()
        self._pulse_until:     float = 0.0
        self._pulse_intensity: float = 0.0
        self._win_until:       float = 0.0
        self._win_next_burst:  float = 0.0
        self._win_burst_on:    bool  = False
        self._win_intensity:   float = 1.0   # set from accuracy at map end
        self._fail_until:      float = 0.0

    def update_v1(self, raw: dict):
        try:
            gp   = raw.get("gameplay", {})
            menu = raw.get("menu", {})

            prev = GameState(
                status=self.state.status,
                hit300=self.state.hit300,
                hit100=self.state.hit100,
                hit50=self.state.hit50,
                hit_miss=self.state.hit_miss,
                health=self.state.health,
                geki=self.state.geki,
                katu=self.state.katu,
                failed=self.state.failed,
            )

            self.state.status   = menu.get("state", 0)
            self.state.health   = gp.get("hp", {}).get("smooth", 0.0)
            hits = gp.get("hits", {})
            self.state.hit300   = hits.get("300", 0)
            self.state.hit100   = hits.get("100", 0)
            self.state.hit50    = hits.get("50", 0)
            self.state.hit_miss = hits.get("0", 0)
            self.state.geki     = hits.get("geki", 0)
            self.state.katu     = hits.get("katu", 0)
            self.state.accuracy = gp.get("accuracy", 0.0)

            self._check_events(prev)
        except Exception as e:
            log.debug(f"v1 parse error: {e}")

    def update_v2(self, raw: dict):
        """Parse v2 data — detects fail from gameplay.failed flip."""
        try:
            play = raw.get("play", {})
            failed_now = play.get("failed", False)
            if failed_now and not self.state.failed:
                self._fail_until = time.time() + 3.0
                log.info("Map failed!")
            self.state.failed = failed_now
        except Exception as e:
            log.debug(f"v2 parse error: {e}")

    def _check_events(self, prev: GameState):
        now = time.time()
        s   = self.state

        # ── Win: state 2→7 (result screen), not failed ──
        if prev.status == 2 and s.status == 7 and not s.failed:
            self._win_intensity  = max(0.20, round(s.accuracy / 100.0, 2))
            self._win_until      = now + WIN_DURATION
            self._win_next_burst = now
            self._win_burst_on   = False
            log.info(f"Map passed! Accuracy: {s.accuracy:.1f}% → win intensity: {self._win_intensity:.0%}")
            return

        # ── Escape / early exit: state 2→anything else, no action ──
        if prev.status == 2 and s.status != 2:
            return

        # ── Fail is handled in update_v2 directly ──
        if prev.status == 2 and s.status != 2:
            return

        if s.status != 2:
            return

        # Miss
        if s.hit_miss > prev.hit_miss:
            self._pulse_until     = now + MISS_DURATION
            self._pulse_intensity = MISS
            log.debug("Miss!")
            return

        # Perfect 300 (geki)
        if s.geki - prev.geki > 0:
            self._pulse_until     = now + PERFECT_DURATION
            self._pulse_intensity = PERFECT
            log.debug("Perfect 300!")
            return

        # Great 300 (regular 300, not geki)
        new_300 = (s.hit300 - prev.hit300) - (s.geki - prev.geki)
        if new_300 > 0:
            self._pulse_until     = now + GREAT_DURATION
            self._pulse_intensity = GREAT
            log.debug("Great 300!")
            return

        # OK 100 (regular 100, not katu)
        new_100 = (s.hit100 - prev.hit100) - (s.katu - prev.katu)
        if new_100 > 0:
            self._pulse_until     = now + OK_DURATION
            self._pulse_intensity = OK
            log.debug("OK 100!")
            return

        # Meh 50
        if s.hit50 - prev.hit50 > 0:
            self._pulse_until     = now + MEH_DURATION
            self._pulse_intensity = MEH
            log.debug("Meh 50!")

    def compute_intensity(self) -> float:
        now = time.time()
        s   = self.state

        # ── Fail: overrides everything, bypasses status check ──
        if now < self._fail_until:
            return FAIL

        # ── Win pattern ──
        if now < self._win_until:
            if now >= self._win_next_burst:
                if self._win_burst_on:
                    self._win_burst_on   = False
                    self._win_next_burst = now + random.uniform(0.1, 0.3)
                else:
                    self._win_burst_on   = True
                    self._win_next_burst = now + random.uniform(0.1, 0.4)
            return self._win_intensity if self._win_burst_on else 0.0

        # ── Not playing ──
        if s.status != 2:
            return IDLE

        # ── Event pulse (hits, misses, slider, spinner) ──
        if now < self._pulse_until:
            return self._pulse_intensity

        return IDLE


# ─── Loops ───────────────────────────────────────────────────────────────────

TOSU_V1_WS = "ws://localhost:24050/ws"
TOSU_V2_WS = "ws://localhost:24050/websocket/v2"


async def tosu_v1_loop(engine: HapticEngine, intensity_ref: list):
    """v1 WebSocket — hits, health, game status. Always processes latest message only."""
    import websockets
    reconnect_delay = 3
    while True:
        try:
            log.info(f"Connecting to tosu v1 at {TOSU_V1_WS} ...")
            async with websockets.connect(TOSU_V1_WS) as ws:
                log.info("Connected to tosu v1!")
                while True:
                    # Drain all pending messages, keep only the latest
                    message = await ws.recv()
                    try:
                        while True:
                            message = ws.messages.popleft()
                    except (IndexError, AttributeError):
                        pass
                    try:
                        data = json.loads(message)
                        engine.update_v1(data)
                        intensity_ref[0] = engine.compute_intensity()
                    except json.JSONDecodeError:
                        pass
        except (ConnectionRefusedError, OSError):
            log.warning(f"Cannot reach tosu v1. Retrying in {reconnect_delay}s...")
            await asyncio.sleep(reconnect_delay)
        except Exception as e:
            log.warning(f"tosu v1 error: {e}. Retrying in {reconnect_delay}s...")
            await asyncio.sleep(reconnect_delay)


async def tosu_v2_loop(engine: HapticEngine):
    """v2 WebSocket — spinner RPM. Optional, fails silently if unavailable."""
    import websockets
    reconnect_delay = 5
    while True:
        try:
            async with websockets.connect(TOSU_V2_WS) as ws:
                log.info("Connected to tosu v2 (spinner data)!")
                async for message in ws:
                    try:
                        data = json.loads(message)
                        engine.update_v2(data)
                    except json.JSONDecodeError:
                        pass
        except (ConnectionRefusedError, OSError):
            await asyncio.sleep(reconnect_delay)
        except Exception:
            await asyncio.sleep(reconnect_delay)


async def vibration_loop(devices: list, intensity_ref: list):
    current   = 0.0
    last_sent = -1.0
    while True:
        target = intensity_ref[0]

        if target >= current:
            current = target
        else:
            current += (target - current) * SMOOTH_DOWN
            if current < 0.04:
                current = 0.0

        intensity = round(current, 2)

        if abs(intensity - last_sent) > 0.01:
            try:
                cmd = DeviceOutputCommand(output_type=OutputType.VIBRATE, value=intensity)
                for device in devices:
                    for feature in device.features.values():
                        if feature.outputs:
                            try:
                                await feature.run_output(cmd)
                            except Exception:
                                pass
                last_sent = intensity
            except Exception as e:
                log.error(f"Vibration command failed: {e}")
        await asyncio.sleep(UPDATE_INTERVAL)


# ─── Main ────────────────────────────────────────────────────────────────────

def parse_args():
    import argparse
    parser = argparse.ArgumentParser(description="VibeLoop — Osu! haptic integration")
    parser.add_argument("--relay",    default=None, help="Relay server WebSocket URL (optional)")
    parser.add_argument("--room",     default=None, help="Room code for relay")
    parser.add_argument("--password", default=None, help="Room password for relay")
    return parser.parse_args()


async def main():
    args = parse_args()
    log.info("=== VibeLoop — Osu! x Hush 2 ===")

    client = ButtplugClient("VibeLoop")

    log.info("Connecting to Intiface Central...")
    try:
        await client.connect(INTIFACE_WS)
    except Exception as e:
        log.error(
            f"Could not connect to Intiface Central: {e}\n"
            "  • Make sure Intiface Central is open\n"
            "  • Make sure you clicked 'Start Server'"
        )
        sys.exit(1)

    log.info("Connected to Intiface Central!")

    # Use already-connected devices, only scan if none found
    if client.devices:
        log.info("Devices already connected, skipping scan.")
    else:
        log.info("No devices connected yet, scanning (3s)...")
        await client.start_scanning()
        await asyncio.sleep(3)
        await client.stop_scanning()

    if not client.devices:
        log.error(
            "No devices found.\n"
            "  • Make sure the Hush 2 is powered on\n"
            "  • Try connecting it in Intiface Central first"
        )
        await client.disconnect()
        sys.exit(1)

    devices = []
    for d in client.devices.values():
        log.info(f"Found device: {d.name}")
        if d.features:
            devices.append(d)

    if not devices:
        log.error("No usable devices found.")
        await client.disconnect()
        sys.exit(1)

    log.info(f"Using {len(devices)} device(s): {', '.join(d.name for d in devices)}")

    engine        = HapticEngine()
    intensity_ref = [0.0]

    tasks = [
        tosu_v1_loop(engine, intensity_ref),
        tosu_v2_loop(engine),
        vibration_loop(devices, intensity_ref),
    ]

    if args.relay and args.room:
        from vibeloop_host import relay_loop
        log.info(f"Relay enabled → {args.relay} | Room: {args.room.upper()}")
        tasks.append(relay_loop(args.relay, args.room, args.password, intensity_ref))
    else:
        log.info("Running locally (no relay). Use --relay / --room to enable remote sync.")

    try:
        await asyncio.gather(*tasks)
    except KeyboardInterrupt:
        log.info("Shutting down...")
    finally:
        for d in devices:
            try:
                await d.stop()
            except Exception:
                pass
        await client.disconnect()
        log.info("Vibration stopped. Goodbye!")


if __name__ == "__main__":
    asyncio.run(main())
