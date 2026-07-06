# WEBFISHING companion mod

A GDWeave script mod that watches the local player and streams bite /
catch / level-up events for the `webfishing.lua` VibeLoop mod. Pure
observer — no gameplay changes, works in multiplayer.

## Install (players)

1. Install [GDWeave](https://thunderstore.io/c/webfishing/p/NotNet/GDWeave/)
   (or use the Hook, Line & Sinker mod manager).
2. Copy the **FullOfSense.VibeLoop** folder (next to this README, contains
   `manifest.json` + `vibeloop.pck`) into `WEBFISHING/GDWeave/Mods/`.
3. Fish. Events land in Godot's user dir
   (`…/Godot/app_userdata/webfishing_2_newver/vibeloop.jsonl`).

## Build from source

The mod is one GDScript file: `vibeloop-bridge/main.gd` (mounted by GDWeave
at `/root`, it watches `player.state` for FISHING_STRUGGLE = bite and
`PlayerData.fish_caught` for catches). Repack it with
[gdsdecomp](https://github.com/GDRETools/gdsdecomp):

```
gdre_tools --headless --pck-create=<dir with mods/FullOfSense.VibeLoop/main.gd> \
  --output=FullOfSense.VibeLoop/vibeloop.pck --pck-version=1 --pck-engine-version=3.5.2
```
