-- @name: Counter-Strike 2 — Punishing 😈
-- @game: Counter-Strike 2
-- @description: Silence while you play — the buzz only comes hard when you die or your team loses the round. Nothing else gets through.
-- @setup: Copy gamestate_integration_vibeloop.cfg (in the mods folder next to this file) into …/Counter-Strike Global Offensive/game/csgo/cfg/, then restart CS2.

-- CS2 pushes its state TO us (Valve's Game State Integration): the cfg file
-- from @setup tells the game to POST JSON to this port whenever something
-- happens. This variant reads only two things from it: your health hitting
-- zero, and the round ending against you.
sources = {
  { id = "gsi", type = "listen", port = 3902 },
}

local last_health = nil
local last_phase = nil       -- round phase (slam the loss only once)

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
    last_health = nil
    return
  end

  local state = data.player.state or {}
  local hp = num(state.health, nil)

  if hp then
    if last_health == nil or hp > last_health then
      last_health = hp
    elseif hp < last_health then
      if hp == 0 then
        vibe.pulse(1.0, 1.2)
        vibe.status("Died 😈")
      end
      -- Taking damage and surviving costs you nothing... yet.
      last_health = hp
    end
  end

  -- Round lost: the other hard consequence. GSI keeps posting during the
  -- "over" phase, so only react to the transition into it.
  local phase = data.round and data.round.phase
  if phase == "over" and last_phase ~= "over" and data.round.win_team then
    if data.player.team ~= data.round.win_team then
      vibe.pulse(0.8, 1.0)
      vibe.status("Round lost 😈")
    else
      vibe.status("Round won — no reward here")
    end
  end
  last_phase = phase
end
