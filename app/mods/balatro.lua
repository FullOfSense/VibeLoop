-- @name: Balatro — Balanced
-- @game: Balatro
-- @description: Scoring builds with the hand, blind wins reward (bosses more), game over stings, money tickles.
-- @setup: Install Steamodded + the VibeLoopBridge mod from mods/companions/balatro/ into your Balatro Mods folder.

-- The bridge writes JSON lines to vibeloop.jsonl in Balatro's save folder.
sources = {
  {
    id = "log",
    type = "file",
    paths = {
      "${APPDATA}/Balatro/vibeloop.jsonl",
      "~/.steam/steam/steamapps/compatdata/2379780/pfx/drive_c/users/steamuser/AppData/Roaming/Balatro/vibeloop.jsonl",
      "~/.local/share/Steam/steamapps/compatdata/2379780/pfx/drive_c/users/steamuser/AppData/Roaming/Balatro/vibeloop.jsonl",
      "~/Library/Application Support/Balatro/vibeloop.jsonl",
    },
  },
}

local function num(x, fallback)
  if type(x) == "number" then return x end
  return fallback
end

function on_message(source, data)
  if data.e == "score" then
    -- Chips climbing toward the blind: buzz grows with progress, capped
    -- so a colossal overshoot doesn't slam full power.
    local progress = num(data.chips, 0) / math.max(num(data.target, 1), 1)
    vibe.pulse(math.min(0.15 + progress * 0.55, 0.8), 0.35)
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
