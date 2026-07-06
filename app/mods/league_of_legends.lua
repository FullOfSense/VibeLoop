-- @name: League of Legends — Balanced
-- @game: League of Legends
-- @description: Damage taken buzzes, kills reward, deaths sting, wins celebrate. Uses Riot's built-in Live Client API.
-- @setup: Nothing to install — just be in a game. Works on every map and mode.

-- Riot ships a local HTTPS API in every League client: it answers on port
-- 2999 while you are in a game (self-signed certificate, hence `insecure`,
-- which VibeLoop only allows for 127.0.0.1).
sources = {
  {
    id = "live",
    url = "https://127.0.0.1:2999/liveclientdata/allgamedata",
    interval = 0.5,
    insecure = true,
  },
}

local LOW_HP = 0.25          -- hum below this health fraction
local last_health = nil
local last_max = nil
local last_kills = nil
local last_deaths = nil
local seen_events = 0
local celebrate_until = 0    -- on_tick wave after a win

local function num(x, fallback)
  if type(x) == "number" then return x end
  return fallback
end

-- The active player appears in allPlayers under riotId ("name#TAG") on
-- current patches and summonerName on older ones — try both.
local function find_me(data)
  local ap = data.activePlayer
  if not ap or type(data.allPlayers) ~= "table" then return nil end
  local names = {}
  if type(ap.riotId) == "string" then names[ap.riotId] = true end
  if type(ap.summonerName) == "string" then names[ap.summonerName] = true end
  for _, p in ipairs(data.allPlayers) do
    if names[p.riotId] or names[p.summonerName] then return p end
  end
  return nil
end

local function is_mine(event_name, data)
  if type(event_name) ~= "string" then return false end
  local ap = data.activePlayer
  if not ap then return false end
  return event_name == ap.riotId or event_name == ap.summonerName
end

local function new_game()
  last_health, last_max, last_kills, last_deaths = nil, nil, nil, nil
  seen_events = 0
end

function on_message(source, data)
  local ap = data.activePlayer
  if not ap or not ap.championStats then return end
  local hp = num(ap.championStats.currentHealth, 0)
  local max_hp = math.max(num(ap.championStats.maxHealth, 1), 1)
  local events = (data.events and data.events.Events) or {}

  -- A shrinking event list means a fresh game started since the last poll.
  if #events < seen_events then new_game() end

  -- Damage taken: pulse scaled by the fraction of max health lost.
  if last_health and hp < last_health - 1 then
    local frac = (last_health - hp) / max_hp
    vibe.pulse(math.min(0.15 + frac * 1.6, 0.85), 0.3)
  end
  last_health, last_max = hp, max_hp

  -- Kills / deaths from the scoreboard: robust even if we miss an event.
  local me = find_me(data)
  if me and me.scores then
    local kills = num(me.scores.kills, 0)
    local deaths = num(me.scores.deaths, 0)
    if last_kills and kills > last_kills then
      vibe.pulse(0.6, 0.5)
      vibe.status("Kill! " .. kills .. "/" .. deaths)
    end
    if last_deaths and deaths > last_deaths then
      vibe.pulse(0.95, 1.2)
      vibe.status("Died… " .. kills .. "/" .. deaths)
    end
    last_kills, last_deaths = kills, deaths
  end

  -- Structured events for the moments the scoreboard can't tell us about.
  for i = seen_events + 1, #events do
    local ev = events[i]
    local name = ev.EventName
    if name == "MultikillEvent" and is_mine(ev.KillerName, data) then
      vibe.pulse(math.min(0.6 + num(ev.KillStreak, 2) * 0.08, 0.95), 0.8)
    elseif (name == "DragonKill" or name == "BaronKill" or name == "HeraldKill")
        and is_mine(ev.KillerName, data) then
      vibe.pulse(0.65, 0.7)
    elseif name == "TurretKilled" and is_mine(ev.KillerName, data) then
      vibe.pulse(0.5, 0.4)
    elseif name == "GameEnd" then
      if ev.Result == "Win" then
        celebrate_until = vibe.now() + 8.0
        vibe.status("VICTORY!")
      else
        vibe.pulse(0.45, 2.0)
        vibe.status("Defeat.")
      end
    end
  end
  seen_events = #events

  -- Low-health tension hum (skipped while dead or celebrating).
  local frac = hp / max_hp
  if vibe.now() >= celebrate_until then
    if hp > 0 and frac < LOW_HP then
      vibe.set(0.08 + 0.15 * (1 - frac / LOW_HP))
    else
      vibe.set(0)
    end
  end
end

function on_tick(now)
  if now < celebrate_until then
    -- Victory wave: 8 s of slow swells.
    vibe.set(0.35 + 0.3 * math.sin((celebrate_until - now) * 2.0))
  elseif celebrate_until > 0 and now < celebrate_until + 0.2 then
    vibe.set(0)
    celebrate_until = 0
  end
end
