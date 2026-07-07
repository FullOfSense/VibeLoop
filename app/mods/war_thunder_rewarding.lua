-- @name: War Thunder — Rewarding 🎁
-- @game: War Thunder
-- @description: Every shot you land purrs — gentle taps for hits, a warm wave for kills. Getting hit or dying does nothing to you.
-- @setup: Put your in-game nickname in MY_NICK inside this file — it's how the mod picks YOUR shots out of the battle feed.

-- Same built-in localhost API as the balanced mod (http://127.0.0.1:8111):
--   /state      — FLIGHT data; ground vehicles report {"valid": false}!
--   /indicators — valid in ANY battle, with army = "tank"/"air"/…
--   /hudmsg     — the battle feed ("X damaged Y") for every vehicle type
-- The feed lists the attacker BEFORE the verb and the victim AFTER it, so
-- with MY_NICK set we reward only the hits YOU land. The feed carries every
-- hit big enough to be announced — damage, crits, fires, kills.
sources = {
  { id = "state", url = "http://127.0.0.1:8111/state", interval = 0.2 },
  { id = "ind", url = "http://127.0.0.1:8111/indicators", interval = 0.5 },
  { id = "feed", url = "http://127.0.0.1:8111/hudmsg?lastEvt=0&lastDmg=0", interval = 1.0 },
}

-- REQUIRED for the full experience: your exact in-game nickname. Without it
-- the mod can't tell your shots from everyone else's, so it falls back to a
-- faint tick for any destruction in the feed.
local MY_NICK = ""

local G_START = 3.0          -- gentler than balanced: G only whispers here
local G_FULL = 9.0
local feed_primed = false    -- very first poll swallows the backlog silently
local last_damage_id = -1
local flying = false         -- /state valid (aircraft telemetry present)

local function num(x, fallback)
  if type(x) == "number" then return x end
  return fallback
end

local VERBS = { "destroyed", "shot down", "damaged", "set afire" }

-- True only when MY_NICK appears BEFORE the verb — i.e. you dealt the hit.
-- (Self-deaths like "has crashed" have no verb from this list, so they and
-- every incoming hit fall through to silence — this mod never punishes.)
local function i_dealt_it(msg)
  if MY_NICK == "" then return false end
  local npos = msg:find(MY_NICK, 1, true)
  if not npos then return false end
  local vpos
  for _, v in ipairs(VERBS) do
    local p = msg:find(v, 1, true)
    if p and (not vpos or p < vpos) then vpos = p end
  end
  return vpos ~= nil and npos < vpos
end

function on_message(source, data)
  if source == "state" then
    flying = data.valid == true
    if flying then
      local g = math.abs(num(data["Ny"], 1.0))
      if g > G_START then
        vibe.set(math.min((g - G_START) / (G_FULL - G_START), 1.0) * 0.3)
        vibe.status(string.format("Pulling %.1f G", g))
      else
        vibe.set(0)
      end
    else
      -- Hangar or a ground/naval battle — /indicators knows which. Never
      -- reset feed tracking here.
      vibe.set(0)
    end
  elseif source == "ind" then
    if data.valid == true then
      if not flying then
        vibe.status("In battle (" .. tostring(data.army or "vehicle")
          .. ") — every hit you land purrs 🎁")
      end
    else
      vibe.status("In hangar — waiting for a battle")
    end
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
        if feed_primed then
          local msg = type(entry.msg) == "string" and entry.msg or ""
          if i_dealt_it(msg) then
            if msg:find("destroyed") or msg:find("shot down") then
              vibe.pulse(0.5, 0.9)           -- kill: a warm, longer wave
            else
              vibe.pulse(0.25, 0.35)         -- your shell connected: soft tap
            end
          elseif MY_NICK == "" then
            -- No nick set: faint tick per destruction so it's not dead air.
            if msg:find("destroyed") or msg:find("shot down") then
              vibe.pulse(0.2, 0.3)
            end
          end
        end
        last_damage_id = id
      end
    end
    feed_primed = true
  end
end
