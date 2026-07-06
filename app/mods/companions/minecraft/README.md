# Minecraft companion mod (Java Edition, Fabric)

A tiny client-side Fabric mod that streams damage/death/level-up events to
`~/.vibeloop/minecraft.jsonl` for the `minecraft.lua` VibeLoop mod. It works
on any server (it changes nothing about gameplay) and with any launcher.

## Install (players)

1. Install the [Fabric loader](https://fabricmc.net/use/installer/) for
   Minecraft 1.21.x and put [Fabric API](https://modrinth.com/mod/fabric-api)
   in your `mods` folder (most modpacks already have both).
2. Drop `vibeloop-bridge-1.0.0.jar` (next to this README) into your
   `mods` folder.
3. Play. The bridge writes to `.vibeloop/minecraft.jsonl` in your home
   folder — the same location for every launcher and OS, so the VibeLoop
   mod finds it automatically.

## Build from source

```
cd vibeloop-bridge
gradle build        # needs JDK 21; the jar lands in build/libs/
```

The whole mod is one Java file: `src/main/java/app/vibeloop/bridge/VibeLoopBridge.java`.
