-- @name: War Thunder — Punishing 😈
-- @game: War Thunder
-- @description: Only pain gets through — every crew loss and enemy shell slams you, ramming hurts by impact speed, dying is a full-power blast. Your own shots and kills: silence.
-- @setup: Put your in-game nickname in MY_NICK inside this file — it's how the mod knows which hits in the battle feed are landing on YOU.

-- Built-in localhost API (http://127.0.0.1:8111). Field behaviour verified
-- against a live tank test drive:
--   /indicators (tanks, 5 Hz here):
--     crew_current DROPS when a hit knocks out a crew member — including
--       hits too small for the battle feed to announce
--     speed collapsing >15 km/h in one poll = you rammed something; that
--       hurts here too, scaled by the speed the impact ate
--   /hudmsg — the battle feed; attacker is named BEFORE the verb, victim
--       AFTER, so MY_NICK tells incoming fire from everything else
-- NOT in the official API (verified absent): sub-damage machine-gun
-- sprinkle and near-misses that don't hurt you — if it took crew, we feel
-- it; if the game didn't register damage, there is nothing to read.
sources = {
  { id = "state", url = "http://127.0.0.1:8111/state", interval = 0.2 },
  { id = "ind", url = "http://127.0.0.1:8111/indicators", interval = 0.2 },
  { id = "feed", url = "http://127.0.0.1:8111/hudmsg?lastEvt=0&lastDmg=0", interval = 1.0 },
}

-- REQUIRED: your exact in-game nickname. Without it the mod can't tell
-- your pain from everyone else's, so the whole feed hits hard instead.
local MY_NICK = ""

local feed_primed = false    -- very first poll swallows the backlog silently
local last_damage_id = -1
local flying = false         -- /state valid (aircraft telemetry present)
local last_crew = nil        -- crew_current (drops = you got hurt)
local last_speed = nil       -- |speed| km/h (collapse = collision)

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
    -- Flight telemetry only tells us we're airborne; this variant deals
    -- exclusively in pain, so no G-load flavour here.
    flying = data.valid == true
  elseif source == "ind" then
    if data.valid == true then
      -- A crew member just died: that's YOUR blood.
      local crew = data.crew_current
      if type(crew) == "number" then
        if last_crew and crew < last_crew then
          vibe.pulse(0.85, 0.5)
        end
        last_crew = crew
      end
      -- Ramming hurts too, scaled by how much speed the wall ate.
      local sp = data.speed
      if type(sp) == "number" then
        sp = math.abs(sp)
        if last_speed and (last_speed - sp) > 15 and last_speed > 12 then
          local impact = last_speed - sp
          vibe.pulse(math.min(0.4 + impact / 60 * 0.4, 0.8), 0.4)
        end
        last_speed = sp
      end
      if not flying then
        vibe.status("In battle (" .. tostring(data.army or "vehicle")
          .. ") — brace for impact 😈")
      end
    else
      last_crew, last_speed = nil, nil
      vibe.status("In hangar — safe... for now")
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
          if role == "victim" then
            if is_kill(msg) then
              vibe.pulse(1.0, 1.2)           -- you died: full power
            else
              vibe.pulse(0.8, 0.6)           -- shell/fire/crit landed on you
            end
          elseif role == "attacker" then
            -- Your kills and hits earn you nothing here.
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
