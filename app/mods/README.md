# VibeLoop Mods

A mod is **one Lua file** in this folder. Drop it in, press 📂 in the app (or
restart it), and it appears in the GAME MOD dropdown. No compilation, no
packaging, no networking code.

## Anatomy

```lua
-- @name: My Game — Spicy
-- @game: My Game
-- @description: One line shown under the dropdown in the app.
-- @setup: What the player must run first, e.g. "Start the game with --api enabled".

-- Where the game data comes from. VibeLoop connects, reconnects, and reports
-- status for every source automatically — mods never handle networking errors.
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
- Test without the game: point a source at a local WebSocket you feed by
  hand — see `core/tests/mock_e2e.rs` for the pattern.

## Shipped mods

| File | Game | Style |
|---|---|---|
| `demo_test.lua` | None — self-running | Repeating 24 s test pattern (pulses → ramp → spikes → wave → rest). Use it to verify toys and host/join sessions without any game |
| `osu_rewarding.lua` | osu! stable & lazer, via [tosu](https://github.com/tosuapp/tosu) | Every hit buzzes, better = stronger; miss/fail hit hardest; win celebration scaled to accuracy |
| `osu_punishing.lua` | osu! stable & lazer, via tosu | Good play = silence; meh hits tickle, misses sting, failing = full power |
