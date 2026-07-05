# VibeLoop Desktop App — Developer Guide

The production VibeLoop app: Rust workspace + Tauri 2 shell + no-build
vanilla-JS frontend + single-file Lua mods. For the user-facing overview see
the [repository README](../README.md); for the security model see
[SECURITY.md](SECURITY.md).

## Architecture

```
app/
├── core/                 vibeloop-core — all logic, UI-independent, fully tested
│   └── src/
│       ├── bus.rs        intensity bus: pulse/base mixing, smoothing, safety cap
│       ├── session.rs    P2P host/join over iroh; room key = KDF(username+password)
│       ├── modengine.rs  Lua mod loader + WebSocket sources with auto-reconnect
│       ├── device.rs     buttplug v10 client → drives all connected toys
│       └── engine.rs     embedded intiface-engine (feature "engine")
├── src-tauri/            Tauri shell: commands, event pumps, lifecycle safety
│   └── src/lib.rs        app state, device bring-up loop, all #[tauri::command]s
├── dist/                 frontend — plain HTML/CSS/JS, no Node, no bundler
└── mods/                 game mods, one .lua each → mods/README.md
```

**Data flow:** `game → mod (Lua) → IntensityBus → toys + session viewers`.
A viewer's app writes received intensity into its own bus, so smoothing,
clamping and device handling are identical on both ends of a session.

**Key design points**

- *Room addressing without a server:* the host's iroh Ed25519 key is derived
  (blake3 KDF) from `lowercase(username) + password`. Viewers derive the same
  public key locally and dial it via n0's free discovery. Wrong password ⇒
  different key ⇒ connection impossible; the password never goes on the wire.
- *Toy engine, both ways:* `device_bringup` first tries Intiface Central at
  `ws://127.0.0.1:12345`; if absent it starts the embedded engine on `:12395`
  (same engine, compiled in — feature `engine`). One buttplug client code path
  either way, retrying forever in the background.
- *Mods can't break the app:* Lua runs behind the `vibe` API only; a runtime
  error stops the mod, zeroes the bus, and surfaces the message in the UI.
  Source WebSockets reconnect every 3 s and report "waiting for game" status.
- *Fail-safe:* every stop path calls `bus.kill()` + `stop_all_devices()`,
  including the Tauri `RunEvent::Exit` hook. Never leave a toy buzzing.

## Development

Prerequisites: Rust stable. Linux additionally:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
  librsvg2-dev libdbus-1-dev libudev-dev build-essential pkg-config
```

```bash
cargo run -p vibeloop                 # run the app (frontend embedded from dist/)
VIBELOOP_MODS_DIR=./mods cargo run -p vibeloop   # use the repo mods dir directly
```

Frontend changes require a rebuild (assets are embedded at compile time).
Mods are read at *runtime* — edit a `.lua`, restart the mod, done.

Mods are discovered in, first match per id wins:
`$VIBELOOP_MODS_DIR` → `<exe dir>/mods` → bundled resources → app data dir
(`~/.local/share/app.vibeloop.desktop/mods` on Linux; seeded from the bundled
mods on first run).

## Tests

Everything below runs **without hardware and without the GUI**:

| Command | Covers |
|---|---|
| `cargo test -p vibeloop-core` | Unit tests (bus, room keys, validation, mod scanning, Lua error containment) + **solo pipeline**: fake tosu → real osu mod → bus → real buttplug client → mock toy speaking the real Buttplug v4 wire protocol |
| `cargo test -p vibeloop-core -- --ignored` | **Live network tests**: host + join a real room via public discovery; full loop host bus → P2P → viewer bus → viewer's mock toy (needs internet, ~5 s) |

CI (`.github/workflows/build.yml`) runs both suites on every tagged release.

## Releasing

Push a tag:

```bash
git tag v0.1.0 && git push origin v0.1.0
```

GitHub Actions builds and drafts a release with the Windows `.exe` installer,
macOS universal `.dmg`, and Linux `.AppImage`/`.deb`. Binaries are unsigned
until certificates are added (users see the usual OS warnings).

## Frontend (dist/)

Single screen, mode-first: SOLO / HOST / JOIN tabs relabel the form so a
field never changes meaning silently (HOST asks for *your* room name, JOIN
for *the host's*). Hosting shows a share card (room + password with reveal
toggle and copy-to-clipboard for stream chat). Talks to Rust exclusively via
`invoke()` commands and events (`log`, `status`, `devices`, `intensity`,
`viewers`, `snapshot`, `session-ended`) — no state of its own beyond the
remembered room name in `localStorage`.
