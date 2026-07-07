-- @name: War Thunder — Punishing 😈
-- @game: War Thunder
-- @description: Getting hit HURTS — enemy shells, fires and crits slam you hard, dying is a full-power blast. Your own kills barely register.
-- @setup: Put your in-game nickname in MY_NICK inside this file — it's how the mod knows which hits in the battle feed are landing on YOU.

-- Same built-in localhost API as the balanced mod (http://127.0.0.1:8111):
--   /state      — FLIGHT data; ground vehicles report {"valid": false}!
--   /indicators — valid in ANY battle, with army = "tank"/"air"/…
--   /hudmsg     — the battle feed ("X damaged Y") for every vehicle type
-- The feed lists the attacker BEFORE the verb and the victim AFTER it, so
-- with MY_NICK set we know exactly when the shell landed on you. The feed
-- only carries hits big enough to be announced (damage, crits, fires,
-- kills) — that's the official API's resolution, small MG pings don't show.
sources = {
  { id = "state", url = "http://127.0.0.1:8111/state", interval = 0.2 },
  { id = "ind", url = "http://127.0.0.1:8111/indicators", interval = 0.5 },
  { id = "feed", url = "http://127.0.0.1:8111/hudmsg?lastEvt=0&lastDmg=0", interval = 1.0 },
}

-- REQUIRED for the full experience: your exact in-game nickname. Without it
-- the mod can't tell your pain from everyone else's, so the whole feed hits
-- hard instead.
local MY_NICK = ""

local G_START = 2.5          -- G-load where the pull starts being felt
local G_FULL = 8.5           -- G-load that maxes the effect
local feed_primed = false    -- very first poll swallows the backlog silently
local last_damage_id = -1
local flying = false         -- /state valid (aircraft telemetry present)

local function num(x, fallback)
  if type(x) == "number" then return x end
  return fallback
end

-- Self-deaths have no victim after the verb — the crasher IS the victim.
local SELF_VERBS = { "has crashed", "has been wrecked" }
local VERBS = { "destroyed", "shot down", "damaged", "set afire" }

-- Returns "attacker", "victim" or nil (nick empty / not mentioned).
local function my_role(msg)
  if MY_NICK == "" then return nil end
  local npos = msg:find(MY_NICK, 1, true)
  if not npos then return nil end
  for _, v in ipairs(SELF_VERBS) do
    if msg:find(v, 1, true) then return "victim" end
  end
  local vpos
  for _, v in ipairs(VERBS) do
    local p = msg:find(v, 1, true)
    if p and (not vpos or p < vpos) then vpos = p end
  end
  if not vpos then return nil end
  if npos < vpos then return "attacker" end
  return "victim"
end

local function is_kill(msg)
  return msg:find("destroyed") or msg:find("shot down")
    or msg:find("has crashed") or msg:find("has been wrecked")
end

function on_message(source, data)
  if source == "state" then
    flying = data.valid == true
    if flying then
      local g = math.abs(num(data["Ny"], 1.0))
      if g > G_START then
        vibe.set(math.min((g - G_START) / (G_FULL - G_START), 1.0) * 0.55)
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
          .. ") — brace for impact 😈")
      end
    else
      vibe.status("In hangar — safe... for now")
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
          local role = my_role(msg)
          if role == "victim" then
            if is_kill(msg) then
              vibe.pulse(1.0, 1.2)           -- you died: full power
            else
              vibe.pulse(0.8, 0.6)           -- shell/fire/crit landed on you
            end
          elseif role == "attacker" then
            vibe.pulse(is_kill(msg) and 0.35 or 0.2, 0.25)  -- barely a treat
          elseif MY_NICK == "" then
            -- No nick set: punish indiscriminately, the whole feed stings.
            if is_kill(msg) then
              vibe.pulse(0.6, 0.5)
            elseif msg:find("damaged") or msg:find("set afire") then
              vibe.pulse(0.45, 0.35)
            end
          end
        end
        last_damage_id = id
      end
    end
    feed_primed = true
  end
end
