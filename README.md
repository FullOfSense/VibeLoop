# VibeLoop

> Feel your game. Share the feeling.

VibeLoop is a cross-platform desktop app (Windows / macOS / Linux) that turns
live game events into haptic feedback on Bluetooth toys — and lets anyone on
the internet feel the same feedback in real time. Built for streamers: host a
room under your name, viewers join with one click, everyone's toy reacts to
your gameplay together.

Built on [buttplug.io](https://buttplug.io) for device support and
[iroh](https://iroh.computer) for peer-to-peer networking. Sponsored with
hardware by [Lovense](https://www.lovense.com).

---

## How it works

```
your game ──▶ mod (one Lua file) ──▶ intensity engine ──▶ your toy
                                          │
                                          ▼  P2P, end-to-end encrypted
                                    viewers' apps ──▶ their toys
```

- **One screen, three modes.** 🎮 **SOLO** — feel your own game.
  📡 **HOST** — pick a room name, share it, stream the feeling.
  💞 **JOIN** — type the host's room name and feel what they feel.
- **No server, no port forwarding, no account.** Connections go directly
  between host and viewer (NAT hole-punching, public relays as automatic
  fallback). Your room name *is* the address.
- **Optional password = private room.** The password is never transmitted —
  it's part of the room's cryptographic identity, so a wrong password simply
  cannot connect. Details in [app/SECURITY.md](app/SECURITY.md).
- **Toys just work.** The app embeds the Intiface engine (Bluetooth LE,
  Lovense USB dongle, Lovense Connect, serial, HID). Already running Intiface
  Central? VibeLoop detects and uses it instead. Every toy buttplug.io
  supports — Lovense, We-Vibe, Kiiroo, and hundreds more.
- **Fail-safe by design.** Every exit path — stop, crash, lost connection,
  app close — zeroes all devices. Intensity is clamped 0–100 % everywhere.

## Supported games

14 games ship with the app: osu! (stable & lazer), League of Legends,
War Thunder, Counter-Strike 2, VRChat, Beat Saber, Factorio, Balatro,
The Binding of Isaac, Team Fortress 2, Don't Starve Together, Minecraft,
R.E.P.O. and WEBFISHING — plus a self-running demo pattern for testing toys
without a game. osu! and War Thunder also come in 🎁 **Rewarding** /
😈 **Punishing** variants. Each mod's in-app setup checklist tells you if
anything needs installing and does it in one click where possible. The
full per-game table lives in [app/mods/README.md](app/mods/README.md).

Adding a game = dropping **one Lua file** into the mods folder (📂 button in
the app). No compiler, no packaging. Full API reference:
[app/mods/README.md](app/mods/README.md).

## Getting started

1. **Install** — grab the installer for your OS from the Releases page
   (`.exe` / `.dmg` / `.AppImage` / `.deb`), or build from source (below).
2. **Turn your toy on.** It appears in the app automatically — use
   *Test buzz* to confirm.
3. **Pick a mode and press start.** For osu!, run
   [tosu](https://github.com/tosuapp/tosu) alongside the game — the app shows
   exactly what it's waiting for.

## Building from source

Rust stable is the only hard requirement. On Linux additionally:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
  librsvg2-dev libdbus-1-dev libudev-dev build-essential pkg-config
```

```bash
cd app
cargo run -p vibeloop                 # run the app
cargo test -p vibeloop-core           # test suite (no hardware needed)
cargo test -p vibeloop-core -- --ignored   # live P2P tests (needs internet)
```

The test suite covers the whole product without hardware: a mock game feeds
the real osu! mod, a mock toy speaks the real buttplug protocol, and the P2P
tests host + join a real room over the live discovery network.
Developer docs: [app/README.md](app/README.md).

## Repository layout

| Path | What |
|---|---|
| [`app/`](app/) | **The VibeLoop desktop app** (Rust + Tauri 2, Lua mods) |
| [`app/mods/`](app/mods/) | Game mods + modding API reference |
| `vibeloop_*.py`, [`Osu/`](Osu/) | Original Python prototype (2025) — kept for reference, superseded by `app/` |

## Roadmap

- [x] Python prototype: osu! → Lovense via Intiface Central, relay-server sync
- [x] **v2 desktop app**: Tauri + Rust, all platforms
- [x] Serverless P2P sessions (username = room, cryptographic passwords)
- [x] Embedded toy engine + Intiface Central auto-detection
- [x] Single-file Lua mod system, osu! mods ported
- [x] CI: installers for Windows / macOS / Linux on every tagged release
- [ ] League of Legends mod
- [ ] Minecraft mod
- [ ] Code signing (Windows / macOS) to remove unsigned-app warnings
- [ ] Per-viewer intensity scaling & host-side viewer management

## License & credits

[MIT](LICENSE).

- **[Lovense](https://www.lovense.com)** — hardware sponsor (special thanks to Luca Fuster)
- **[buttplug.io](https://buttplug.io) / [Intiface](https://intiface.com)** — device engine (Nonpolynomial Labs)
- **[iroh](https://iroh.computer)** — P2P connections (n0)
- **[tosu](https://github.com/tosuapp/tosu)** — osu! game state
- **[Tauri](https://tauri.app)** — app shell
- Built by [FullOfSense](https://github.com/FullOfSense) — HBO-ICT student, Netherlands
