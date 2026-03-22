# VibeLoop

> Real-time haptic feedback for Lovense devices, driven by in-game events.

VibeLoop is an open source personal project that maps live game state data to vibration patterns across one or more Lovense devices simultaneously. The goal is a shared, immersive experience between two users — each on their own machine, each with their own device — reacting to the same game in real time. The architecture supports both local and remote setups, making it naturally applicable to long-distance use.

This project is built on top of [Intiface Central](https://intiface.com/central) / [Buttplug.io](https://buttplug.io) and is intended as an honest, transparent record of what works, what doesn't, and where the stack holds up under unconventional real-world use.

---

## Hardware Requirements

To run VibeLoop in its intended two-device configuration you will need:

- **2× Lovense USB Bluetooth Dongle** — one per machine, required for device communication
- **Lovense Lush 3** — first device, connected to machine 1
- **Lovense Hush 2 (1.75")** — second device, connected to machine 2
- Two computers running the game integrations simultaneously

A single-device setup is also possible with one dongle and one device, useful for testing individual game integrations before moving to multi-device sync.

---

## Supported Games & APIs

| Game | API / Method | Status |
|---|---|---|
| osu! | [tosu](https://github.com/tosuapp/tosu) | ✅ Working |
| League of Legends | [Live Client API](https://developer.riotgames.com/docs/lol#game-client-api) | 🔜 Planned |
| Minecraft | Modding framework (Fabric / Forge TBD) | 🔜 Planned |

Each integration has its own README with setup instructions and haptic mapping details.

---

## Installation

Each game integration has its own setup guide. See the relevant README for full instructions:

- **osu!** → [Osu/README_osu.md](Osu/README_osu.md)
- League of Legends → *(coming soon)*
- Minecraft → *(coming soon)*

**Shared requirements across all integrations:**

- Python 3 with `buttplug` and `websockets` packages
- [Intiface Central](https://intiface.com/central) installed and running as the device server
- A Lovense USB Bluetooth Dongle per machine
- Your Lovense device(s) connected in Intiface Central before running any script

---

## Remote Sync

VibeLoop supports remote play — one machine runs the game and broadcasts haptic intensity, while any number of people around the world connect and feel the same feedback on their own devices.

### How it works

```
Your machine                    Cloud / Local              Friend's machine
─────────────────               ───────────────            ────────────────
osu! + vibeloop_osu.py   →→→   vibeloop_server.py  →→→   vibeloop_client.py
     (host mode)               (relay server)                  (client)
          │                                                        │
       Hush 2                                                   Lush 3
```

### Files

| File | Role |
|---|---|
| `vibeloop_server.py` | Relay server — runs in the cloud or locally |
| `vibeloop_host.py` | Standalone host broadcaster (imported by game scripts) |
| `vibeloop_client.py` | Runs on the viewer's machine, drives their local device |

### Usage

**Start the relay server (once):**
```bash
python3 vibeloop_server.py
```

**Host a room (your machine, with game running):**
```bash
python3 Osu/vibeloop_osu_rewarding.py --relay ws://YOUR_SERVER:8765 --room ROOMCODE
```

**Join as a client (friend's machine):**
```bash
python3 vibeloop_client.py --server ws://YOUR_SERVER:8765 --room ROOMCODE
```

Rooms support an optional password:
```bash
--password secret
```

### GUI Launcher

A graphical launcher is included for convenience:
```bash
python3 vibeloop_gui.py
```

Requires: `pip install customtkinter`

The GUI lets you select a game, choose local or host/client mode, enter relay details, and launch with one click. Live logs are streamed directly into the interface.

---

## Roadmap

### Phase 1 — Setup & Documentation ✅
- [x] Repository created
- [x] README written
- [x] Hardware received (Hush 2 + Lush 3 + 2× USB Dongle, sponsored by Lovense)
- [x] Decide on programming language (Python)
- [x] Set up project structure

### Phase 2 — Osu! Integration ✅
- [x] Connect to tosu WebSocket
- [x] Map hit judgements and rhythm events to vibration intensity
- [x] Map fail / pass events to haptic patterns (collapse animation detection)
- [x] Win intensity scaled to final accuracy
- [x] Multi-device support (all Intiface-connected devices vibrate in sync)
- [x] Two variants: Rewarding and Punishing

### Phase 3 — Remote Sync ✅
- [x] WebSocket relay server with room codes and optional passwords
- [x] Host mode — broadcasts game intensity to relay
- [x] Client mode — receives intensity and drives local device
- [x] Tested locally (Linux host + Windows client, Hush + Lush in sync)
- [ ] Deploy relay server to cloud (Railway / Render)
- [ ] Test across internet (two different networks)

### Phase 4 — GUI Launcher ✅
- [x] CustomTkinter desktop app
- [x] Game selector with variant picker
- [x] Local / Host / Join mode switching
- [x] Live log output
- [ ] Client count display in host mode

### Phase 5 — League of Legends Integration
- [ ] Connect to Live Client API
- [ ] Map game events (damage, kills, abilities) to vibration patterns
- [ ] Test single and multi-device behaviour

### Phase 6 — Minecraft Integration
- [ ] Select modding framework (Fabric or Forge)
- [ ] Map in-game events (damage, crafting, environment) to vibration patterns
- [ ] Test single and multi-device behaviour

### Phase 7 — Multi-Device Sync & Refinement
- [ ] Deploy relay server permanently
- [ ] Stress test simultaneous two-device, two-machine communication over internet
- [ ] Document latency, connection stability, and sync reliability findings
- [ ] Refactor and clean up codebase

---

## License

This project is licensed under the [MIT License](LICENSE).

---

## Credits & Acknowledgements

- **[Lovense](https://www.lovense.com)** — hardware sponsor. Special thanks to Luca Fuster.
- **[tosu](https://github.com/tosuapp/tosu)** — osu! game state reader
- **[Intiface Central](https://intiface.com/central) / [Buttplug.io](https://buttplug.io)** — device communication layer
- **[Riot Games](https://developer.riotgames.com)** — League of Legends Live Client API
- Built by [FullOfSense](https://github.com/FullOfSense) — HBO-ICT student, Netherlands
