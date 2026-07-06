# Factorio companion mod

Copy the `vibeloop-bridge` folder into your Factorio mods directory:

- Windows: `%APPDATA%\Factorio\mods\`
- Linux: `~/.factorio/mods/`
- macOS: `~/Library/Application Support/factorio/mods/`

Enable it in the in-game Mods menu. It writes events to
`script-output/vibeloop.jsonl`, which the `factorio.lua` VibeLoop mod tails.

On Factorio **1.1** change `"factorio_version": "2.0"` to `"1.1"` in
`info.json` — the code supports both.

Note: like every Factorio mod it disables achievements for that save.
