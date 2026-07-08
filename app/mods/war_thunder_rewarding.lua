-- @name: War Thunder — Rewarding 🎁
-- @game: War Thunder
-- @description: Feel everything YOU do — cannon shots thump, ramming jolts by impact speed, your hits tap, kills send a warm wave. Taking damage only tickles.
-- @setup: Put your in-game nickname in MY_NICK inside this file — it's how the mod picks YOUR shots out of the battle feed.

-- Built-in localhost API (http://127.0.0.1:8111). Field behaviour verified
-- against a live tank test drive:
--   /indicators (tanks, 5 Hz here):
--     first_stage_ammo DROPS by 1 the moment the cannon fires (an increase
--       is the rack replenishing — ignored)
--     speed collapsing >15 km/h in one poll = you rammed something (hard
--       braking measured ~6 km/h per poll, a wall hit ~30+)
--     crew_current DROPS when a hit knocks out a crew member — including
--       hits too small for the battle feed to announce
--   /state — flight-only; gives aircraft the gentle G-pull
--   /hudmsg — the battle feed; attacker is named BEFORE the verb, victim
--       AFTER, so MY_NICK tells your shots from everyone else's
-- NOT in the official API (verified absent): machine-gun bursts, smoke
-- deployment, and near-miss artillery/bombs that don't damage you.
sources = {
  { id = "state", url = "http://127.0.0.1:8111/state", interval = 0.2 },
  { id = "ind", url = "http://127.0.0.1:8111/indicators", interval = 0.2 },
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
local last_ammo = nil        -- first-stage rack count (tank cannon shots)
local last_crew = nil        -- crew_current (drops = you got hurt)
local last_speed = nil       -- |speed| km/h (collapse = collision)

local function num(x, fallback)
  if type(x) == "number" then return x end
  return fallback
end

local VERBS = { "destroyed", "shot down", "damaged", "set afire" }

-- "attacker" when MY_NICK appears before the verb, "victim" after it.
local function my_role(msg)
  if MY_NICK == "" then return nil end
  local npos = msg:find(MY_NICK, 1, true)
  if not npos then return nil end
  local vpos
  for _, v in ipairs(VERBS) do
    local p = msg:find(v, 1, true)
    if p and (not vpos or p < vpos) then vpos = p end
  end
  if not vpos then return nil end
  if npos < vpos then return "attacker" end
  return "victim"
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
      vibe.set(0)
    end
  elseif source == "ind" then
    if data.valid == true then
      -- Cannon shot: the ready rack lost a shell.
      local ammo = data.first_stage_ammo
      if type(ammo) == "number" then
        if last_ammo and ammo < last_ammo then
          vibe.pulse(0.35, 0.25)
        end
        last_ammo = ammo
      end
      -- You got hurt: a crew member was knocked out. Catches even hits
      -- the battle feed never announces.
      local crew = data.crew_current
      if type(crew) == "number" then
        if last_crew and crew < last_crew then
          vibe.pulse(0.2, 0.3)
        end
        last_crew = crew
      end
      -- Collision: speed collapsed far faster than brakes can manage.
      -- Jolt scales with how much speed the impact ate.
      local sp = data.speed
      if type(sp) == "number" then
        sp = math.abs(sp)
        if last_speed and (last_speed - sp) > 15 and last_speed > 12 then
          local impact = last_speed - sp
          vibe.pulse(math.min(0.2 + impact / 60 * 0.3, 0.5), 0.3)
        end
        last_speed = sp
      end
      if not flying then
        vibe.status("In battle (" .. tostring(data.army or "vehicle")
          .. ") — every move purrs 🎁")
      end
    else
      last_ammo, last_crew, last_speed = nil, nil, nil
      vibe.status("In hangar — waiting for a battle")
    end
  elseif source == "feed" then
    if type(data.damage) ~= "table" then return end
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
          if role == "attacker" then
            if msg:find("destroyed") or msg:find("shot down") then
              vibe.pulse(0.5, 0.9)           -- kill: a warm, longer wave
            else
              vibe.pulse(0.25, 0.35)         -- your shell connected
            end
          elseif role == "victim" then
            vibe.pulse(0.2, 0.25)            -- damaged, but only a tickle
          elseif MY_NICK == "" then
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
