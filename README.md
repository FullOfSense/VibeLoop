# VibeLoop

> Real-time haptic feedback for Lovense devices, driven by in-game events.

VibeLoop is an open source personal project that maps live game state data to vibration patterns across one or more Lovense devices simultaneously. The goal is a shared, immersive experience between two users — each on their own machine, each with their own device — reacting to the same game in real time. The architecture supports both local and remote setups, making it naturally applicable to long-distance use.

This project is built on top of the [Lovense Developer API](https://developer.lovense.com) and is intended as an honest, transparent record of what works, what doesn't, and where the API holds up under unconventional real-world use.

> **Status:** In setup phase. Hardware received. Development begins once primary machine is repaired (est. 2 weeks).

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
| Osu! | [tosu](https://github.com/tosuapp/tosu) | ✅ Working |
| League of Legends | [Live Client API](https://developer.riotgames.com/docs/lol#game-client-api) | 🔜 Planned |
| Minecraft | Modding framework (Fabric / Forge TBD) | 🔜 Planned |

---

## Installation

Each game integration has its own setup guide. See the relevant README for full instructions:

- **osu!** → [README_osu.md](README_osu.md)
- League of Legends → *(coming soon)*
- Minecraft → *(coming soon)*

**Shared requirements across all integrations:**

- Python 3 with `buttplug` and `websockets` packages
- [Intiface Central](https://intiface.com/central) installed and running as the device server
- A Lovense USB Bluetooth Dongle per machine
- Your Lovense device(s) connected in Intiface Central before running any script

---

## Roadmap

### Phase 1 — Setup & Documentation ✅
- [x] Repository created
- [x] README written
- [x] Hardware received (Hush 2 + 2× USB Dongle, sponsored by Lovense)
- [x] Decide on programming language (Python)
- [x] Set up project structure

### Phase 2 — Osu! Integration ✅
- [x] Connect to tosu websocket
- [x] Map hit judgements and rhythm events to vibration intensity
- [x] Map fail / pass events to haptic patterns
- [x] Multi-device support (all Intiface-connected devices vibrate in sync)
- [x] Test single-device response
- [ ] Test two-device sync across two machines

### Phase 3 — League of Legends Integration
- [ ] Connect to Live Client API
- [ ] Map game events (damage, kills, abilities) to vibration patterns
- [ ] Test single and multi-device behaviour

### Phase 4 — Minecraft Integration
- [ ] Select modding framework (Fabric or Forge)
- [ ] Map in-game events (damage, crafting, environment) to vibration patterns
- [ ] Test single and multi-device behaviour

### Phase 5 — Multi-Device Sync & Refinement
- [ ] Stress test simultaneous two-device, two-machine communication
- [ ] Document latency, connection stability, and sync reliability findings
- [ ] Refactor and clean up codebase
- [ ] Write final API findings report

---

## License

This project is licensed under the [MIT License](LICENSE).

---

## Credits & Acknowledgements

- **[Lovense](https://www.lovense.com)** — hardware sponsor and API provider.
- **[tosu](https://github.com/tosuapp/tosu)** — Osu! game state reader
- **[Riot Games](https://developer.riotgames.com)** — League of Legends Live Client API
- Built by [FullOfSense](https://github.com/FullOfSense) — HBO-ICT student, Netherlands
