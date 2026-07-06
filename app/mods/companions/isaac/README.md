# The Binding of Isaac companion mod

Requires Repentance or Repentance+ (the Lua modding API).

Copy the `vibeloop bridge` folder into the game's mods directory:

- Windows: `C:\Program Files (x86)\Steam\steamapps\common\The Binding of Isaac Rebirth\mods\`
- Linux: `~/.steam/steam/steamapps/common/The Binding of Isaac Rebirth/mods/`
- macOS: `~/Library/Application Support/Steam/steamapps/common/The Binding of Isaac Rebirth/mods/`

Enable "VibeLoop Bridge" in the in-game Mods menu (and remember: mods
disable achievements until you beat Mom once per save).

The bridge writes marker lines into the game's `log.txt`, which the
`binding_of_isaac.lua` VibeLoop mod tails — no network, no files of its own.
