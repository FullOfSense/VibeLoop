#!/usr/bin/env python3
"""
VibeLoop Relay Server
=====================
Relays vibration intensity from a host to any number of connected clients.

Rooms are created by the host with an optional password.
Clients join by room code and provide the password if required.

Usage:
    python3 vibeloop_server.py

Environment variables (optional):
    PORT        — port to listen on (default: 8765)
"""

import asyncio
import json
import logging
import os
import sys
import time
from dataclasses import dataclass, field

import websockets
from websockets.server import WebSocketServerProtocol

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    handlers=[logging.StreamHandler(sys.stdout)],
)
log = logging.getLogger("VibeLoop.Server")

PORT = int(os.environ.get("PORT", 8765))


# ─── Room Management ─────────────────────────────────────────────────────────

@dataclass
class Room:
    code:      str
    password:  str | None         # None = no password required
    host:      WebSocketServerProtocol | None = None
    clients:   set                = field(default_factory=set)
    intensity: float              = 0.0
    created_at: float             = field(default_factory=time.time)

    def is_open(self) -> bool:
        return self.password is None

    def check_password(self, pw: str | None) -> bool:
        if self.password is None:
            return True
        return pw == self.password

    def client_count(self) -> int:
        return len(self.clients)


rooms: dict[str, Room] = {}


def get_or_create_room(code: str, password: str | None) -> Room:
    if code not in rooms:
        rooms[code] = Room(code=code, password=password)
        log.info(f"Room created: {code} (password: {'yes' if password else 'no'})")
    return rooms[code]


def cleanup_room(code: str):
    if code in rooms:
        room = rooms[code]
        if room.host is None and room.client_count() == 0:
            del rooms[code]
            log.info(f"Room removed: {code}")


# ─── Message Helpers ──────────────────────────────────────────────────────────

def msg(**kwargs) -> str:
    return json.dumps(kwargs)


async def send(ws: WebSocketServerProtocol, **kwargs):
    try:
        await ws.send(msg(**kwargs))
    except Exception:
        pass


async def broadcast_to_clients(room: Room, intensity: float):
    if not room.clients:
        return
    payload = msg(type="intensity", intensity=intensity)
    await asyncio.gather(
        *[client.send(payload) for client in room.clients],
        return_exceptions=True,
    )


# ─── Connection Handler ───────────────────────────────────────────────────────

async def handler(ws: WebSocketServerProtocol):
    role = None
    room_code = None

    try:
        # ── Handshake ──
        raw = await asyncio.wait_for(ws.recv(), timeout=10)
        data = json.loads(raw)

        role      = data.get("role")       # "host" or "client"
        room_code = data.get("room", "").strip().upper()
        password  = data.get("password") or None

        if role not in ("host", "client") or not room_code:
            await send(ws, type="error", message="Invalid handshake. Provide role and room.")
            return

        # ── Host joining ──
        if role == "host":
            room = get_or_create_room(room_code, password)

            if room.host is not None:
                await send(ws, type="error", message="Room already has a host.")
                return

            room.host = ws
            await send(ws, type="joined", role="host", room=room_code,
                       clients=room.client_count())
            log.info(f"Host connected → room {room_code}")

            async for raw_msg in ws:
                try:
                    data = json.loads(raw_msg)
                    if data.get("type") == "intensity":
                        intensity = max(0.0, min(1.0, float(data["intensity"])))
                        room.intensity = intensity
                        await broadcast_to_clients(room, intensity)
                    elif data.get("type") == "ping":
                        await send(ws, type="pong", clients=room.client_count())
                except (json.JSONDecodeError, KeyError, ValueError):
                    pass

        # ── Client joining ──
        elif role == "client":
            if room_code not in rooms:
                await send(ws, type="error", message="Room not found.")
                return

            room = rooms[room_code]

            if not room.check_password(password):
                await send(ws, type="error", message="Incorrect password.")
                return

            room.clients.add(ws)
            await send(ws, type="joined", role="client", room=room_code,
                       intensity=room.intensity)
            log.info(f"Client connected → room {room_code} ({room.client_count()} clients)")

            # Notify host of new client count
            if room.host:
                await send(room.host, type="info", clients=room.client_count())

            # Keep alive — clients don't send anything, just receive
            await ws.wait_closed()

    except asyncio.TimeoutError:
        await send(ws, type="error", message="Handshake timeout.")
    except websockets.exceptions.ConnectionClosedOK:
        pass
    except websockets.exceptions.ConnectionClosedError:
        pass
    except Exception as e:
        log.warning(f"Handler error: {e}")
    finally:
        # ── Cleanup ──
        if room_code and room_code in rooms:
            room = rooms[room_code]
            if role == "host" and room.host is ws:
                room.host = None
                log.info(f"Host disconnected from room {room_code}")
                # Notify clients the host left
                await asyncio.gather(
                    *[send(c, type="host_disconnected") for c in room.clients],
                    return_exceptions=True,
                )
            elif role == "client":
                room.clients.discard(ws)
                log.info(f"Client disconnected from room {room_code} ({room.client_count()} remaining)")
                if room.host:
                    await send(room.host, type="info", clients=room.client_count())
            cleanup_room(room_code)


# ─── Main ─────────────────────────────────────────────────────────────────────

async def main():
    log.info(f"VibeLoop Relay Server starting on port {PORT}")
    async with websockets.serve(handler, "0.0.0.0", PORT):
        log.info(f"Listening on ws://0.0.0.0:{PORT}")
        await asyncio.Future()  # run forever


if __name__ == "__main__":
    asyncio.run(main())
