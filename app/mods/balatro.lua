-- @name: Balatro — Balanced
-- @game: Balatro
-- @description: Every action buzzes — each scoring pop ticks in a cascade, draws and discards tap, blind wins reward (bosses more), game over stings.
-- @setup: Install Steamodded + the VibeLoopBridge mod from mods/companions/balatro/ into your Balatro Mods folder.

-- The bridge writes JSON lines to vibeloop.jsonl in Balatro's save folder.
sources = {
  {
    id = "log",
    type = "file",
    -- Fast tail: scoring pops ride the count-up animation, so read the
    -- bridge file at 10 Hz to keep buzz-to-animation lag imperceptible.
    interval = 0.1,
    paths = {
      "${APPDATA}/Balatro/vibeloop.jsonl",
      -- Proton on Linux: found in whichever Steam library holds the game.
      "${STEAM_LIBRARIES}/compatdata/2379780/pfx/drive_c/users/steamuser/AppData/Roaming/Balatro/vibeloop.jsonl",
      "~/Library/Application Support/Balatro/vibeloop.jsonl",
    },
  },
}

local function num(x, fallback)
  if type(x) == "number" then return x end
  return fallback
end

local combo = 0    -- pops in the current count-up (escalates the buzz)
local combo_t = 0  -- when the last pop landed

function on_message(source, data)
  if data.e == "score" then
    -- Chips climbing toward the blind: buzz grows with progress, capped
    -- so a colossal overshoot doesn't slam full power.
    local progress = num(data.chips, 0) / math.max(num(data.target, 1), 1)
    vibe.pulse(math.min(0.15 + progress * 0.55, 0.8), 0.35)
  elseif data.e == "cardpop" then
    -- A card just lifted and showed its "+X" during the count-up. Chips
    -- scale with the points added; mult pops run warmer, xMult pops are
    -- the spicy ones; and the whole cascade escalates as it builds.
    local now = vibe.now()
    if now - combo_t > 1.5 then combo = 0 end
    combo = combo + 1
    combo_t = now
    local t, base = data.t, 0
    if t == "x_mult" or t == "h_x_mult" then
      base = 0.3
    elseif t == "mult" or t == "h_mult" then
      base = 0.22
    elseif t == "dollars" or t == "money" then
      base = 0.2
    else -- chips and anything new
      base = 0.15 + math.min(num(data.amt, 0) * 0.004, 0.15)
    end
    vibe.pulse(math.min(base + combo * 0.02, 0.55), 0.12)
  elseif data.e == "pop" then
    -- Legacy event from bridge 1.1.0 installs; superseded by cardpop.
    vibe.pulse(0.18, 0.1)
  elseif data.e == "play" then
    combo = 0
    vibe.pulse(0.3, 0.2)
  elseif data.e == "draw" then
    -- One soft tap per card dealt to your hand.
    vibe.pulse(0.12, 0.08)
  elseif data.e == "discard" then
    vibe.pulse(math.min(0.2 + num(data.n, 1) * 0.03, 0.35), 0.2)
  elseif data.e == "use" then
    -- Tarot / planet / spectral used.
    vibe.pulse(0.3, 0.25)
  elseif data.e == "money" then
    if num(data.d, 0) > 0 then
      vibe.pulse(0.2, 0.12)
    end
  elseif data.e == "ante" then
    vibe.pulse(0.45, 0.5)
    vibe.status("Ante " .. num(data.n, 0))
  elseif data.e == "state" then
    local s = data.s
    if s == "ROUND_EVAL" then
      -- Blind beaten; bosses hit harder.
      vibe.pulse(data.boss and 0.85 or 0.6, data.boss and 1.0 or 0.6)
      vibe.status(data.boss and "Boss blind down!" or "Blind beaten!")
    elseif s == "GAME_OVER" then
      vibe.pulse(0.9, 1.5)
      vibe.status("Game over")
    elseif s == "NEW_ROUND" or s == "SHOP" then
      vibe.set(0)
    end
  end
end
