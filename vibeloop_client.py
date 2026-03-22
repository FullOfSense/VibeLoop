#!/usr/bin/env python3
"""
VibeLoop Client
===============
Connects to a VibeLoop relay server, joins a room by code,
and drives all locally connected Lovense devices via Intiface Central.

Usage:
    python3 vibeloop_client.py --server ws://HOST_SERVER:8765 --room MYROOM
    python3 vibeloop_client.py --server ws://HOST_SERVER:8765 --room MYROOM --password secret

Arguments:
    --server    WebSocket URL of the relay server (get this from the host)
    --room      Room code shared by the host
    --password  Password if the room is protected
"""

import asyncio
import json
import logging
import sys
import argparse

import websockets
from buttplug import ButtplugClient, DeviceOutputCommand, OutputType

# ─── Logging ─────────────────────────────────────────────────────────────────

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    handlers=[logging.StreamHandler(sys.stdout)],
)
log = logging.getLogger("VibeLoop.Client")

INTIFACE_WS     = "ws://localhost:12345"
UPDATE_INTERVAL = 0.05


# ─── Vibration loop ───────────────────────────────────────────────────────────

async def vibration_loop(devices: list, intensity_ref: list):
    current   = 0.0
    last_sent = -1.0
    SMOOTH_DOWN = 0.35

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


# ─── Relay receive loop ───────────────────────────────────────────────────────

async def relay_loop(server_url: str, room: str, password: str | None, intensity_ref: list):
    room = room.strip().upper()
    reconnect_delay = 5

    while True:
        try:
            log.info(f"Connecting to relay server at {server_url} ...")
            async with websockets.connect(server_url) as ws:

                # Handshake
                handshake = {"role": "client", "room": room}
                if password:
                    handshake["password"] = password
                await ws.send(json.dumps(handshake))

                response = json.loads(await ws.recv())
                if response.get("type") == "error":
                    log.error(f"Server error: {response.get('message')}")
                    return
                if response.get("type") == "joined":
                    log.info(f"Joined room: {room}")
                    intensity_ref[0] = response.get("intensity", 0.0)

                async for raw in ws:
                    try:
                        data = json.loads(raw)

                        if data.get("type") == "intensity":
                            intensity_ref[0] = max(0.0, min(1.0, float(data["intensity"])))

                        elif data.get("type") == "host_disconnected":
                            log.info("Host disconnected. Intensity set to 0.")
                            intensity_ref[0] = 0.0

                    except (json.JSONDecodeError, ValueError):
                        pass

        except (ConnectionRefusedError, OSError):
            log.warning(f"Cannot reach relay server. Retrying in {reconnect_delay}s...")
            intensity_ref[0] = 0.0
            await asyncio.sleep(reconnect_delay)
        except websockets.exceptions.ConnectionClosedError:
            log.warning(f"Connection lost. Retrying in {reconnect_delay}s...")
            intensity_ref[0] = 0.0
            await asyncio.sleep(reconnect_delay)
        except Exception as e:
            log.warning(f"Relay error: {e}. Retrying in {reconnect_delay}s...")
            intensity_ref[0] = 0.0
            await asyncio.sleep(reconnect_delay)


# ─── Main ─────────────────────────────────────────────────────────────────────

def parse_args():
    parser = argparse.ArgumentParser(description="VibeLoop Client")
    parser.add_argument("--server",   required=True, help="Relay server WebSocket URL")
    parser.add_argument("--room",     required=True, help="Room code")
    parser.add_argument("--password", default=None,  help="Room password if required")
    return parser.parse_args()


async def main():
    args = parse_args()

    log.info("=== VibeLoop Client ===")

    # Connect to Intiface
    client = ButtplugClient("VibeLoop Client")
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

    if client.devices:
        log.info("Devices already connected, skipping scan.")
    else:
        log.info("No devices found, scanning (3s)...")
        await client.start_scanning()
        await asyncio.sleep(3)
        await client.stop_scanning()

    if not client.devices:
        log.error(
            "No devices found.\n"
            "  • Make sure your device is powered on\n"
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

    intensity_ref = [0.0]

    try:
        await asyncio.gather(
            relay_loop(args.server, args.room, args.password, intensity_ref),
            vibration_loop(devices, intensity_ref),
        )
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
