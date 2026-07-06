-- @name: Counter-Strike 2 — Balanced
-- @game: Counter-Strike 2
-- @description: Damage buzzes, kills reward, flashes and fire are felt, round wins celebrate. Official Game State Integration.
-- @setup: Copy gamestate_integration_vibeloop.cfg (in the mods folder next to this file) into …/Counter-Strike Global Offensive/game/csgo/cfg/, then restart CS2.

-- CS2 pushes its state TO us (Valve's Game State Integration): the cfg file
-- from @setup tells the game to POST JSON to this port whenever something
-- happens. VibeLoop listens on 127.0.0.1 only.
sources = {
  { id = "gsi", type = "listen", port = 3902 },
}

local last_health = nil
local last_round_kills = nil
local burning_or_planted = 0   -- sustained hum level from ambient states

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
    -- Spectating: stay quiet, keep trackers unset until we respawn.
    last_health, last_round_kills = nil, nil
    vibe.set(0)
    return
  end

  local state = data.player.state or {}
  local hp = num(state.health, nil)

  if hp then
    -- Fresh round (or first payload): full health resets the baseline.
    if last_health == nil or hp > last_health then
      last_health = hp
    elseif hp < last_health then
      local dmg = last_health - hp
      if hp == 0 then
        vibe.pulse(0.95, 1.0)
        vibe.status("Died")
      else
        vibe.pulse(math.min(0.2 + (dmg / 100) * 1.1, 0.85), 0.35)
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

  -- Flashbang: `flashed` is 0–255 and decays as vision returns.
  local flashed = num(state.flashed, 0)
  local burning = num(state.burning, 0)
  local hum = 0
  if flashed > 0 then hum = math.max(hum, (flashed / 255) * 0.35) end
  if burning > 0 then hum = math.max(hum, 0.45) end

  -- Light tension hum while the bomb is down.
  if data.round and data.round.bomb == "planted" then
    hum = math.max(hum, 0.08)
  end
  vibe.set(hum)

  -- Round over: celebrate a win for your side.
  if data.round and data.round.phase == "over" and data.round.win_team then
    if data.player.team == data.round.win_team then
      vibe.pulse(0.7, 1.2)
      vibe.status("Round won!")
    end
    last_round_kills = nil
  end
end
