# VibeLoop Mods

A mod is **one Lua file** in this folder. Drop it in, press 📂 in the app (or
restart it), and it appears in the GAME MOD dropdown. No compilation, no
packaging, no networking code.

Every shipped mod comes with a **setup checklist** in the app: select the
mod and VibeLoop checks on disk whether the game and every dependency are
actually in place. Steps it can do itself (copying our companion bridges
into the game) get an **Install** button; steps only you can do (installing
BepInEx, enabling OSC, a launch option) get a **Guide ↗** link and a
Re-check button. When the summary says *READY TO PLAY*, it will work.

Want to connect a game that isn't here yet? Read the full guide:
[`docs/MODDING.md`](../docs/MODDING.md).

## Anatomy

```lua
-- @name: My Game — Spicy
-- @game: My Game
-- @description: One line shown under the dropdown in the app.
-- @setup: What the player must run first, e.g. "Start the game with --api enabled".

-- Where the game data comes from. VibeLoop connects, reconnects, and reports
-- status for every source automatically — mods never handle networking errors.
-- Besides WebSockets there are three more source types: `poll` (local HTTP
-- APIs like League/War Thunder), `listen` (games that POST to you, like CS2)
-- and `osc` (VRChat) — see docs/MODDING.md for all fields.
sources = {
  { id = "main", url = "ws://127.0.0.1:12345/game-data" },
}

-- Called for every JSON message a source delivers, already decoded to a table.
function on_message(source, data)
  if data.player_took_damage then
    vibe.pulse(0.8, 0.25)          -- 80% intensity for 0.25 s
  end
end

-- Optional. Called ~20×/second with seconds elapsed since the mod started.
function on_tick(now)
  -- for sustained/timed patterns, e.g. celebrations
end
```

A mod may also skip `sources` entirely and run on `on_tick` alone — that's
how `demo_test.lua` generates its pattern with no game attached.

## Header keys

Read from `-- @key: value` comments in the first 20 lines, **without
executing the file** — a broken mod still shows up in the list and reports
its error only when started.

| Key | Shown |
|---|---|
| `@name` | Mod list entry (falls back to the file name) |
| `@game` | Prefix in the dropdown, groups variants of the same game |
| `@description` | Grey line under the dropdown |
| `@setup` | Orange ⚙ hint under the dropdown — what to run before playing |

## The `vibe` API

| Function | Effect |
|---|---|
| `vibe.pulse(level, seconds)` | One-shot pulse (0.0–1.0). A new pulse replaces the previous one. |
| `vibe.set(level)` | Sustained level, stays until you set something else. `vibe.set(0)` to release. |
| `vibe.now()` | Seconds (fractional) since the mod started. |
| `vibe.log(msg)` | Line in the app's log panel. |
| `vibe.status(msg)` | Replaces the app's one-line status text. |

Final intensity is `max(set level, active pulse)`, smoothed (instant rise,
soft fall), capped at 0–1. When hosting, that exact value is what every
viewer feels — mods never need to know whether a session is running.

## Rules of the road

- Sources reconnect forever with a 3 s delay; the app shows *"Waiting for
  game link…"* until the game/tool is up. Mods stay silent about it.
- If a source sends faster than the mod processes, stale frames are dropped —
  design for *absolute* state (totals, not deltas) where you control the
  sender.
- A Lua runtime error stops the mod safely (intensity zeroed) and shows the
  error in the app. `error("...")` is a legitimate way to bail out.
- Mods run sandboxed: `math`/`string`/`table`/`utf8` and the `vibe` API only —
  no `io`, `os`, or `require`. All I/O goes through declared `sources`.
- Test without the game: point a source at a local WebSocket you feed by
  hand — see `core/tests/mock_e2e.rs` for the pattern.

## Shipped mods

| File | Game | Style |
|---|---|---|
| `demo_test.lua` | None — self-running | Repeating 24 s test pattern (pulses → ramp → spikes → wave → rest). Use it to verify toys and host/join sessions without any game |
| `osu_rewarding.lua` | osu! stable & lazer, via [tosu](https://github.com/tosuapp/tosu) | Every hit buzzes, better = stronger; miss/fail hit hardest; win celebration scaled to accuracy |
| `osu_punishing.lua` | osu! stable & lazer, via tosu | Good play = silence; meh hits tickle, misses sting, failing = full power |
| `league_of_legends.lua` | League of Legends (built-in Live Client API, nothing to install) | Damage taken buzzes by % lost, kills reward, deaths sting, low HP hums, victory celebrates |
| `war_thunder.lua` | War Thunder (built-in localhost API, nothing to install) | G-forces pull; battle-feed hits and kills buzz — set `MY_NICK` in the file to feel only your own |
| `war_thunder_punishing.lua` | War Thunder (same API; `MY_NICK` required) | 😈 Only pain: crew losses and enemy shells slam, ramming hurts by impact, dying is full power; your own shots and kills are silent |
| `war_thunder_rewarding.lua` | War Thunder (same API; `MY_NICK` required) | 🎁 Gun-ready thump when your reload finishes, ramming jolts by impact, your hits tap, kills wave; taking damage only tickles |
| `counterstrike2.lua` | Counter-Strike 2 (copy the cfg from `companions/cs2/` once) | Damage, kills, flashes, fire, bomb tension, round wins |
| `counterstrike2_rewarding.lua` | Counter-Strike 2 (same cfg) | 🎁 Every trigger pull patters (spray = rattle), kills reward with streaks, round ends celebrate; damage and death only tickle |
| `counterstrike2_punishing.lua` | Counter-Strike 2 (same cfg) | 😈 Silence while you play — dying slams full power, losing the round hits hard, nothing else gets through |
| `vrchat.lua` | VRChat (enable OSC in-game) | An avatar float parameter — e.g. a Contact Receiver named `VibeLoop` — drives intensity directly |
| `beat_saber.lua` | Beat Saber, via the DataPuller mod | Cuts tick, misses sting, combo milestones, low energy hums, finish celebrates by accuracy |
| `factorio.lua` | Factorio (bridge mod in `companions/factorio/`) | Damage buzzes, deaths sting, research rewards, rocket launches celebrate |
| `balatro.lua` | Balatro (Steamodded bridge in `companions/balatro/`) | Scoring builds toward the blind, boss blinds hit harder, game over stings |
| `binding_of_isaac.lua` | The Binding of Isaac (bridge in `companions/isaac/`) | Heart damage scales the sting, boss kills and new floors reward |
| `team_fortress2.lua` | Team Fortress 2 (`-condebug` launch option, set `MY_NICK` in the file) | Your kills reward (crits extra), your deaths sting |
| `dont_starve_together.lua` | Don't Starve Together (client bridge in `companions/dst/`) | Hits buzz by health lost, death stings; works on any server |
| `minecraft.lua` | Minecraft Java + Fabric (prebuilt bridge jar in `companions/minecraft/`) | Damage buzzes by hearts lost, death stings, level-ups reward; any launcher, any server |
| `repo.lua` | R.E.P.O. (BepInEx + prebuilt dll in `companions/repo/`) | Your damage buzzes, deaths sting, heals tickle, extractions celebrate — teammates' pain ignored |
| `webfishing.lua` | WEBFISHING (GDWeave + prebuilt pck in `companions/webfishing/`) | Feel the bite the instant a fish strikes; catches and rod level-ups celebrate |
