# Modding games for VibeLoop

This is the guide for connecting **any** game to VibeLoop. A mod is one Lua
file in the `mods/` folder — no compiler, no SDK, no networking code. The app
handles connecting, reconnecting, "is the game running?" detection and all
error handling; your Lua only decides *what should be felt when*.

If you just want to use the shipped mods, see [`mods/README.md`](../mods/README.md).
This document is for making new ones.

## The one question that matters

**How does game state get out of the game?** Every game falls into one of
five patterns, and the first four map directly onto a VibeLoop source type:

| Pattern | Source type | Example games |
|---|---|---|
| The game runs a local HTTP API you can poll | `poll` | League of Legends (`https://127.0.0.1:2999`), War Thunder (`http://127.0.0.1:8111`) |
| The game POSTs state to a URL you give it | `listen` | Counter-Strike 2 / Dota 2 (Game State Integration) |
| The game broadcasts OSC over UDP | `osc` | VRChat (avatar parameters on port 9001) |
| A community tool bridges the game to a WebSocket | `ws` | osu! (tosu), Beat Saber (DataPuller) |
| The game (or a tiny in-game script) writes a file | `file` | Factorio (script-output), TF2 (console.log), Isaac (log.txt), Balatro, DST |
| The game has its own mod SDK and nothing above | write a tiny companion mod in the game that speaks one of the above | Minecraft (Fabric), REPO (BepInEx), BTD6 (Mod Helper) |

The `file` pattern is the great equalizer: **any** game whose modding API can
print a line — even just to its own debug log — can talk to VibeLoop. The
in-game side of that is usually under 40 lines; see `mods/companions/`.

How to find out which one your game is:

1. Search "`<game> local API`", "`<game> game state integration`",
   "`<game> websocket overlay`". Stream-overlay and stats-tracker projects
   have usually done the discovery for you — check what *they* connect to.
2. While the game runs, look at what it listens on:
   `ss -tlnp | grep <game>` (Linux), `netstat -ano` (Windows),
   `lsof -iTCP -sTCP:LISTEN` (macOS).
3. If the game is moddable, the modding community's Discord knows in
   minutes what would take you days to reverse-engineer.

**Never** read game memory or inject into a process — besides being fragile,
it gets people banned in anything with anti-cheat. If a game offers no data
channel at all, the honest answer is "not moddable yet" (see the coverage
table at the bottom).

## Declaring sources

```lua
sources = {
  -- WebSocket (type inferred from the URL)
  { id = "live", url = "ws://127.0.0.1:2946/BSDataPuller/LiveData" },

  -- HTTP polling (inferred from http/https). interval in seconds.
  -- insecure=true accepts a self-signed certificate; VibeLoop only
  -- permits it for 127.0.0.1/localhost URLs.
  { id = "api", url = "https://127.0.0.1:2999/liveclientdata/allgamedata",
    interval = 0.5, insecure = true },

  -- HTTP listener: accepts POSTs on 127.0.0.1:<port>, each body is a message.
  { id = "gsi", type = "listen", port = 3902 },

  -- OSC over UDP on 127.0.0.1:<port>. Each OSC message arrives as
  -- { addr = "/avatar/parameters/X", args = { 0.7 } }.
  { id = "vrc", type = "osc", port = 9001 },

  -- Tail a file. Give one `path` or a `paths` list of candidates (per-OS
  -- install locations); `~` and `${ENV_VARS}` expand. Tailing starts at the
  -- END of the file (history is never replayed) and survives truncation and
  -- the file disappearing. JSON lines arrive decoded; any other line
  -- arrives as { line = "the raw text" }.
  { id = "log", type = "file", paths = {
      "~/.factorio/script-output/vibeloop.jsonl",
      "${APPDATA}/Factorio/script-output/vibeloop.jsonl",
  } },
}
```

Every payload that arrives on any source is JSON-decoded and handed to your
`on_message(source, data)` with `source` set to the `id`. Sources that can't
connect (or stop hearing from the game) show *"waiting for the game"* in the
app and keep retrying forever — your Lua never sees any of that.

`on_tick(now)` runs ~20×/second regardless of sources; use it for timed
patterns (celebrations, safety releases). A mod may even be tick-only, like
`demo_test.lua`.

## The `vibe` API

| Function | Effect |
|---|---|
| `vibe.pulse(level, seconds)` | One-shot pulse (0.0–1.0); a new pulse replaces the old |
| `vibe.set(level)` | Sustained level until you change it; `vibe.set(0)` releases |
| `vibe.now()` | Seconds since the mod started |
| `vibe.log(msg)` | Line in the app's log panel |
| `vibe.status(msg)` | One-line status under the intensity meter |

Output is `max(set, pulse)`, smoothed, clamped 0–1, capped by the user's
safety slider. You cannot exceed what the user allowed no matter what you
write.

## Patterns that make mods feel right (and not break)

These come from the shipped mods — read them as worked examples.

**Track totals, not events.** Prefer "kills went from 3 to 4" over parsing a
kill event: totals survive missed frames. The League mod uses the scoreboard
for kills/deaths and only uses the event list for things totals can't tell it
(multikills, game end). Slow sources drop stale frames by design, so state
you compare *must* be cumulative.

**Prime before you react.** The first payload after connecting describes the
past, not something that just happened. Swallow it to set your baselines,
react from the second one on (`war_thunder.lua`'s `feed_primed`,
`counterstrike2.lua`'s `last_health == nil`).

**Detect resets.** Counters go backwards when a new game/round/battle starts.
A shrinking event list (League), an ID below your high-water mark
(War Thunder), or health jumping back to full (CS2) means "reset your
baselines now" — otherwise the next match starts numb or replays the old one.

**Make sure it's *you*.** Spectator and killfeed data includes other players.
CS2 compares `player.steamid` against `provider.steamid`; War Thunder greps
the feed for your nickname. Nothing is more confusing than buzzing when your
teammate gets shot.

**Release on silence.** If the "back to zero" packet can be lost (UDP/OSC) or
the game can vanish mid-buzz, add an `on_tick` timeout that zeroes after ~1–2 s
without updates (`vrchat.lua`). VibeLoop itself zeroes everything if your mod
crashes or is stopped, so this is only about lost *game* packets.

**Be defensive about fields.** Games change their JSON between patches.
`local function num(x, d) if type(x)=="number" then return x end return d end`
at the top of every shipped mod is not decoration — a missing field must
degrade to "no effect", not crash the mod (a Lua error stops your mod, with
intensity safely zeroed, until restarted).

## Testing without the game

Fake the game, not the toy. Each source type is trivial to feed by hand:

```bash
# listen (CS2-style): pretend to be the game with curl
curl -s -X POST http://127.0.0.1:3902 \
  -d '{"provider":{"steamid":"1"},"player":{"steamid":"1","state":{"health":40}}}'

# poll: serve a canned state file
python3 -m http.server 8111   # with a ./state file in the directory

# osc (VRChat-style): one packet from Python
python3 -c "
import socket, struct
def pad(b): return b + b'\0' * ((4 - len(b) % 4) % 4)
msg = pad(b'/avatar/parameters/VibeLoop\0') + pad(b',f\0') + struct.pack('>f', 0.8)
socket.socket(socket.AF_INET, socket.SOCK_DGRAM).sendto(msg, ('127.0.0.1', 9001))"
```

For automated tests, `core/tests/tier_a_mods.rs` shows full fake games for
every source type, plus how to unit-test a mod's logic by calling
`on_message` with fixture JSON directly.

## Rules of the sandbox

Mods run with `math`, `string`, `table`, `utf8` and the `vibe` API — no `io`,
`os`, `require`, or network access of their own. All I/O goes through
declared `sources`, which are plainly visible at the top of the file. That's
what makes mods safe to share: the worst a hostile mod can do is buzz
(clamped) and write log lines.

## Game coverage

Status of every game on the current roadmap. "Companion" means a small
open-source plugin in the game's own modding framework that bridges to a
VibeLoop source; they live under `mods/companions/`.

| Game | Channel | Status |
|---|---|---|
| osu! | tosu → `ws` | ✅ shipped |
| League of Legends | Live Client API → `poll` | ✅ shipped |
| War Thunder | localhost:8111 → `poll` | ✅ shipped |
| Counter-Strike 2 | Game State Integration → `listen` | ✅ shipped (cfg in `mods/companions/cs2/`) |
| VRChat | OSC → `osc` | ✅ shipped |
| Beat Saber | DataPuller → `ws` | ✅ shipped |
| Factorio | bridge mod → script-output → `file` | ✅ shipped (`mods/companions/factorio/`) |
| Balatro | Steamodded bridge → `file` | ✅ shipped (`mods/companions/balatro/`) |
| The Binding of Isaac | bridge mod → log.txt → `file` | ✅ shipped (`mods/companions/isaac/`) |
| Team Fortress 2 | `-condebug` console.log → `file` | ✅ shipped (launch option only, no mod) |
| Don't Starve Together | client bridge → client_log.txt → `file` | ✅ shipped (`mods/companions/dst/`) |
| Minecraft | Fabric companion → `ws` | 🔜 tier C |
| REPO | BepInEx companion → `ws` | 🔜 tier C |
| Bloons TD 6 | Mod Helper companion → `ws` | 🔜 tier C |
| Webfishing | GDWeave companion → `ws` | 🔜 tier C |
| Deep Rock Galactic | companion feasibility open | 🔜 tier C |
| Satisfactory | companion / dedicated-server API | 🔜 tier C |
| Palworld | dedicated-server REST only | ⚠️ partial at best |
| Sea of Thieves | none (anti-cheat, no API) | ❌ not moddable safely |
| Crab Champions | none | ❌ no data channel |
| Generation Zero | none | ❌ no data channel |
| Cash Cleaner Simulator | none | ❌ no data channel |

A ❌ turns into a ✅ the day the game ships an API, a modding framework, or
the community builds a bridge — the coverage above is about data channels,
never about VibeLoop.

## Shipping your mod

Header comments make your mod present itself in the app:

```lua
-- @name: My Game — Balanced
-- @game: My Game
-- @description: One line shown under the dropdown.
-- @setup: The one thing the player must do first (install X / copy the cfg).
```

Keep `@setup` honest and singular — if setup takes more than one step, ship a
companion folder with a README like `mods/companions/cs2/`. Then just share
the `.lua` file; dropping it into the mods folder is the whole install.
