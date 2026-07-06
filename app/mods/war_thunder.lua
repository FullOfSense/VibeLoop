-- @name: War Thunder — Balanced
-- @game: War Thunder
-- @description: G-forces pull, hits and kills from the battle feed buzz. Uses the game's built-in localhost API.
-- @setup: Optional: put your in-game nickname in MY_NICK inside this file so only YOUR hits and kills count.

-- War Thunder always serves telemetry on http://127.0.0.1:8111 during a
-- battle — no mod or setting needed. `state` carries flight data ~5×/s,
-- `hudmsg` carries the battle feed ("X destroyed Y").
sources = {
  { id = "state", url = "http://127.0.0.1:8111/state", interval = 0.2 },
  { id = "feed", url = "http://127.0.0.1:8111/hudmsg?lastEvt=0&lastDmg=0", interval = 1.0 },
}

-- Set this to your exact in-game nickname to only feel events that mention
-- you. Left empty, every destruction in the feed gives a small buzz.
local MY_NICK = ""

local G_START = 2.5          -- G-load where the pull starts being felt
local G_FULL = 8.5           -- G-load that maxes the effect
local feed_primed = false    -- first poll swallows the backlog silently
local last_damage_id = -1
local g_level = 0

local function num(x, fallback)
  if type(x) == "number" then return x end
  return fallback
end

function on_message(source, data)
  if source == "state" then
    if data.valid ~= true then
      g_level = 0
      vibe.set(0)
      -- Between battles the feed counter starts over.
      last_damage_id, feed_primed = -1, false
      vibe.status("In hangar — waiting for a battle")
      return
    end
    -- Aircraft G-load ("Ny"). Ground vehicles simply don't send it.
    local g = math.abs(num(data["Ny"], 1.0))
    if g > G_START then
      g_level = math.min((g - G_START) / (G_FULL - G_START), 1.0) * 0.55
      vibe.status(string.format("Pulling %.1f G", g))
    else
      g_level = 0
    end
    vibe.set(g_level)
  elseif source == "feed" then
    if type(data.damage) ~= "table" then return end
    -- IDs restarting below our high-water mark = a new battle began.
    local batch_max = -1
    for _, entry in ipairs(data.damage) do
      batch_max = math.max(batch_max, num(entry.id, -1))
    end
    if batch_max >= 0 and batch_max < last_damage_id then
      last_damage_id = -1
    end
    for _, entry in ipairs(data.damage) do
      local id = num(entry.id, -1)
      if id > last_damage_id then
        -- First poll of a battle: swallow the backlog instead of replaying it.
        if feed_primed then
          local msg = type(entry.msg) == "string" and entry.msg or ""
          local mine = MY_NICK == "" or msg:find(MY_NICK, 1, true) ~= nil
          if mine then
            if msg:find("destroyed") or msg:find("shot down") then
              vibe.pulse(MY_NICK == "" and 0.35 or 0.7, 0.45)
            elseif msg:find("damaged") or msg:find("set afire")
                or msg:find("critically") then
              vibe.pulse(MY_NICK == "" and 0.25 or 0.5, 0.3)
            end
          end
        end
        last_damage_id = id
      end
    end
    feed_primed = true
  end
end
