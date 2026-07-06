# Don't Starve Together companion mod

Copy the `vibeloop-bridge` folder into DST's mods directory:

- Windows: `C:\Program Files (x86)\Steam\steamapps\common\Don't Starve Together\mods\`
- Linux: `~/.steam/steam/steamapps/common/Don't Starve Together/mods/`
- macOS: `~/Library/Application Support/Steam/steamapps/common/Don't Starve Together/mods/`

Enable it under Mods → Client Mods. It is **client-only**: it works on any
server, no server-side install, nobody else needs it.

It prints marker lines that land in `client_log.txt`, which the
`dont_starve_together.lua` VibeLoop mod tails.
