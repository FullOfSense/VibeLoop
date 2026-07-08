-- @name: Counter-Strike 2 — Rewarding 🎁
-- @game: Counter-Strike 2
-- @description: Every trigger pull patters (spray = continuous rattle), kills reward with streak bonus, round ends celebrate; damage and death only tickle.
-- @setup: Copy gamestate_integration_vibeloop.cfg (in the mods folder next to this file) into …/Counter-Strike Global Offensive/game/csgo/cfg/, then restart CS2.

-- CS2 pushes its state TO us (Valve's Game State Integration): the cfg file
-- from @setup tells the game to POST JSON to this port whenever something
-- happens (up to ~10×/s). The cfg subscribes player_weapons, so the active
-- weapon's ammo_clip is in every update — a drop means YOU fired, and the
-- size of the drop is how many rounds left the barrel since last update.
sources = {
  { id = "gsi", type = "listen", port = 3902 },
}

local last_health = nil
local last_round_kills = nil
local last_weapon = nil      -- active weapon name (switch = reset tracker)
local last_clip = nil        -- its ammo_clip (drop = shots fired)
local last_phase = nil       -- round phase (fire round-over feedback once)

local function num(x, fallback)
  if type(x) == "number" then return x end
  return fallback
end

-- When you die and spectate a teammate, CS2 keeps sending *their* state.
-- Only react to your own: the `provider` block is always you.
local function is_me(data)
  return data.player and data.provider
    and data.player.steamid == data.provider.steamid
end

function on_message(source, data)
  if not data.player then return end

  if not is_me(data) then
    last_health, last_round_kills = nil, nil
    last_weapon, last_clip = nil, nil
    vibe.set(0)
    return
  end

  -- Shots: the active weapon's clip shrank since the last update. A single
  -- tap ticks once; holding the trigger lands a drop of 2–6 rounds per
  -- update, which reads as a continuous rattle at full auto.
  local weapons = data.player.weapons
  if type(weapons) == "table" then
    local active_name, clip
    for _, w in pairs(weapons) do
      if type(w) == "table" and w.state == "active" then
        active_name = w.name
        clip = num(w.ammo_clip, nil)
      end
    end
    if active_name then
      if active_name == last_weapon and clip and last_clip
        and clip < last_clip then
        local rounds = last_clip - clip
        vibe.pulse(math.min(0.15 + rounds * 0.04, 0.35), 0.12)
      end
      -- Weapon switch or reload refill: just move the baseline, no buzz.
      last_weapon, last_clip = active_name, clip
    end
  end

  local state = data.player.state or {}
  local hp = num(state.health, nil)

  if hp then
    if last_health == nil or hp > last_health then
      last_health = hp
    elseif hp < last_health then
      local dmg = last_health - hp
      if hp == 0 then
        vibe.pulse(0.3, 0.5)   -- death is only a firm tap here
        vibe.status("Died")
      else
        vibe.pulse(math.min(0.15 + (dmg / 100) * 0.25, 0.35), 0.25)
        vibe.status(hp .. " HP")
      end
      last_health = hp
    end
  end

  -- Kills this round (resets every round, so watch for increases only).
  local rk = num(state.round_kills, 0)
  if last_round_kills and rk > last_round_kills then
    vibe.pulse(math.min(0.5 + (rk - 1) * 0.12, 0.9), 0.5)
    vibe.status(rk .. " kill" .. (rk > 1 and "s" or "") .. " this round!")
  end
  last_round_kills = rk

  -- Flash and fire: a gentler hum than balanced.
  local flashed = num(state.flashed, 0)
  local burning = num(state.burning, 0)
  local hum = 0
  if flashed > 0 then hum = math.max(hum, (flashed / 255) * 0.2) end
  if burning > 0 then hum = math.max(hum, 0.25) end
  if data.round and data.round.bomb == "planted" then
    hum = math.max(hum, 0.08)
  end
  vibe.set(hum)

  -- Round over: wins celebrate properly, losses still get a soft nod —
  -- the round ending is an event either way. GSI keeps posting during the
  -- "over" phase, so only react to the transition into it.
  local phase = data.round and data.round.phase
  if phase == "over" and last_phase ~= "over" and data.round.win_team then
    if data.player.team == data.round.win_team then
      vibe.pulse(0.6, 1.0)
      vibe.status("Round won!")
    else
      vibe.pulse(0.25, 0.4)
      vibe.status("Round over")
    end
    last_round_kills = nil
  end
  last_phase = phase
end
