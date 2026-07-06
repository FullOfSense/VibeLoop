# R.E.P.O. companion mod

A BepInEx plugin that streams the **local player's** damage/death/heal
events (plus completed extractions) to `~/.vibeloop/repo.jsonl` for the
`repo.lua` VibeLoop mod. Teammates' damage is ignored on purpose.

## Install (players)

1. Install **BepInExPack** — easiest via r2modman / Thunderstore Mod
   Manager, or manually from
   [thunderstore.io/c/repo](https://thunderstore.io/c/repo/p/BepInEx/BepInExPack/).
2. Drop `VibeLoopBridge.REPO.dll` (next to this README) into
   `BepInEx/plugins/` (with r2modman: the profile's plugins folder).
3. Play. Events land in `.vibeloop/repo.jsonl` in your (or the Proton
   prefix's) home folder.

## Build from source

```
cd vibeloop-bridge
dotnet build -c Release -p:GameDir="<path to steamapps/common/REPO>" \
             -p:BepInExDir="<path to BepInEx/core>"
```

The plugin hooks `PlayerHealth.Hurt/Death/Heal` (filtered by
`PlayerAvatar.isLocal`) and `ExtractionPoint.StateComplete` via Harmony
postfixes — read `VibeLoopBridge.cs`, it's one file.
