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
| Osu! | [Gosumemory](https://github.com/l3lackShark/gosumemory) | 🔜 Planned |
| League of Legends | [Live Client API](https://developer.riotgames.com/docs/lol#game-client-api) | 🔜 Planned |
| Minecraft | Modding framework (Fabric / Forge TBD) | 🔜 Planned |

---

## Installation

> ⚠️ **This section is a placeholder.** Installation instructions will be added once the first working integration is complete.

General requirements will likely include:

- A supported OS (Windows primary, Linux TBD)
- The Lovense Connect app installed and running
- Node.js / Python / (language TBD) runtime
- A Lovense USB Bluetooth Dongle per machine

---

## Roadmap

### Phase 1 — Setup & Documentation ✅
- [x] Repository created
- [x] README written
- [x] Hardware received (Hush 2 + 2× USB Dongle, sponsored by Lovense)
- [ ] Decide on programming language
- [ ] Set up project structure

### Phase 2 — Osu! Integration 🎯 *(target: within 60 days of hardware receipt)*
- [ ] Connect to Gosumemory websocket
- [ ] Map hit accuracy and rhythm events to vibration intensity
- [ ] Test single-device response
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
- **[Gosumemory](https://github.com/l3lackShark/gosumemory)** — Osu! game state reader
- **[Riot Games](https://developer.riotgames.com)** — League of Legends Live Client API
- Built by [FullOfSense](https://github.com/FullOfSense) — HBO-ICT student, Netherlands
