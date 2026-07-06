# Counter-Strike 2 companion config

CS2 only sends game state to programs you explicitly allow, via a config
file in the game's own folder (Valve's official Game State Integration —
no anti-cheat concerns, it's the same mechanism tournament HUDs use).

**Install (one time):** copy `gamestate_integration_vibeloop.cfg` into

- Windows: `C:\Program Files (x86)\Steam\steamapps\common\Counter-Strike Global Offensive\game\csgo\cfg\`
- Linux: `~/.steam/steam/steamapps/common/Counter-Strike Global Offensive/game/csgo/cfg/`

then restart CS2. That's it — the game will start POSTing state to
VibeLoop (127.0.0.1 only) whenever the `counterstrike2.lua` mod is running.

**Uninstall:** delete the cfg file.
