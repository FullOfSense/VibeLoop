-- @name: Factorio — Balanced
-- @game: Factorio
-- @description: Biter bites buzz, deaths sting, finished research rewards, rocket launches celebrate.
-- @setup: Install the vibeloop-bridge mod from mods/companions/factorio/ into your Factorio mods folder.

-- The bridge writes JSON lines to script-output/vibeloop.jsonl; we tail it
-- wherever Factorio keeps its user data on this OS.
sources = {
  {
    id = "log",
    type = "file",
    paths = {
      "~/.factorio/script-output/vibeloop.jsonl",
      "${APPDATA}/Factorio/script-output/vibeloop.jsonl",
      "~/Library/Application Support/factorio/script-output/vibeloop.jsonl",
    },
  },
}

local celebrate_until = 0

local function num(x, fallback)
  if type(x) == "number" then return x end
  return fallback
end

function on_message(source, data)
  if data.e == "dmg" then
    -- f = fraction of max health lost in one hit.
    local f = num(data.f, 0)
    vibe.pulse(math.min(0.2 + f * 1.4, 0.85), 0.3)
  elseif data.e == "died" then
    vibe.pulse(0.95, 1.5)
    vibe.status("You died!")
  elseif data.e == "research" then
    vibe.pulse(0.5, 0.8)
    vibe.status("Research complete")
  elseif data.e == "rocket" then
    celebrate_until = vibe.now() + 6.0
    vibe.status("Rocket launched! 🚀")
  end
end

function on_tick(now)
  if now < celebrate_until then
    vibe.set(0.35 + 0.3 * math.sin((celebrate_until - now) * 3.0))
  elseif celebrate_until > 0 and now < celebrate_until + 0.2 then
    vibe.set(0)
    celebrate_until = 0
  end
end
