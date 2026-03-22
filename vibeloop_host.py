#!/usr/bin/env python3
"""
VibeLoop Host
=============
Runs alongside your normal vibeloop_osu.py (or any future integration).
Connects to the VibeLoop relay server and broadcasts intensity in real time
so remote clients around the world can sync their devices.

Usage:
    python3 vibeloop_host.py --server ws://YOUR_SERVER:8765 --room MYROOM
    python3 vibeloop_host.py --server ws://YOUR_SERVER:8765 --room MYROOM --password secret

Arguments:
    --server    WebSocket URL of the relay server
    --room      Room code (shared with your viewers, case-insensitive)
    --password  Optional password viewers must enter to join
"""

import asyncio
import json
import logging
import sys
import argparse

import websockets

# ─── Logging ─────────────────────────────────────────────────────────────────

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    handlers=[logging.StreamHandler(sys.stdout)],
)
log = logging.getLogger("VibeLoop.Host")


# ─── Shared intensity ref ─────────────────────────────────────────────────────
# This module exposes intensity_ref so vibeloop_osu.py (or any integration)
# can import and write to it. The broadcast loop reads from it continuously.

intensity_ref = [0.0]


# ─── Relay broadcast loop ─────────────────────────────────────────────────────

async def relay_loop(server_url: str, room: str, password: str | None, intensity_ref: list = None):
    if intensity_ref is None:
        intensity_ref = globals().get("intensity_ref", [0.0])
    room = room.strip().upper()
    reconnect_delay = 5
    last_sent = -1.0

    while True:
        try:
            log.info(f"Connecting to relay server at {server_url} ...")
            async with websockets.connect(server_url) as ws:

                # Handshake
                handshake = {"role": "host", "room": room}
                if password:
                    handshake["password"] = password
                await ws.send(json.dumps(handshake))

                response = json.loads(await ws.recv())
                if response.get("type") == "error":
                    log.error(f"Server error: {response.get('message')}")
                    return
                if response.get("type") == "joined":
                    log.info(f"Hosting room: {room} | Clients connected: {response.get('clients', 0)}")

                # Broadcast loop
                while True:
                    intensity = round(intensity_ref[0], 2)

                    if abs(intensity - last_sent) > 0.01:
                        await ws.send(json.dumps({"type": "intensity", "intensity": intensity}))
                        last_sent = intensity

                    # Ping every 5 seconds to get client count
                    # (handled via a separate task below)
                    await asyncio.sleep(0.05)

        except (ConnectionRefusedError, OSError):
            log.warning(f"Cannot reach relay server. Retrying in {reconnect_delay}s...")
            await asyncio.sleep(reconnect_delay)
        except websockets.exceptions.ConnectionClosedError:
            log.warning(f"Relay connection lost. Retrying in {reconnect_delay}s...")
            await asyncio.sleep(reconnect_delay)
        except Exception as e:
            log.warning(f"Relay error: {e}. Retrying in {reconnect_delay}s...")
            await asyncio.sleep(reconnect_delay)


# ─── Argument parsing / standalone run ───────────────────────────────────────

def parse_args():
    parser = argparse.ArgumentParser(description="VibeLoop Host — relay broadcaster")
    parser.add_argument("--server",   required=True, help="Relay server WebSocket URL")
    parser.add_argument("--room",     required=True, help="Room code")
    parser.add_argument("--password", default=None,  help="Optional room password")
    return parser.parse_args()


async def main():
    args = parse_args()
    log.info("=== VibeLoop Host ===")
    log.info(f"Room: {args.room.upper()} | Password: {'set' if args.password else 'none'}")
    await relay_loop(args.server, args.room, args.password)


if __name__ == "__main__":
    asyncio.run(main())
