# Balatro companion mod

Requires [Steamodded](https://github.com/Steamodded/smods) (which needs
[lovely](https://github.com/ethangreen-dev/lovely-injector)) — the standard
Balatro modding setup.

Copy the `VibeLoopBridge` folder into your Balatro `Mods` directory:

- Windows: `%APPDATA%\Balatro\Mods\`
- Linux (Steam/Proton): `~/.steam/steam/steamapps/compatdata/2379780/pfx/drive_c/users/steamuser/AppData/Roaming/Balatro/Mods/`
- macOS: `~/Library/Application Support/Balatro/Mods/`

It writes events to `vibeloop.jsonl` next to the Mods folder, which the
`balatro.lua` VibeLoop mod tails.

Balatro has no official event API, so this bridge watches game state each
frame (chips, money, ante, game state) — a game patch could move those
fields; if the mod goes silent after an update, check for a bridge update.
