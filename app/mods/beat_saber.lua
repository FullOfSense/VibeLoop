-- @name: Beat Saber — Balanced
-- @game: Beat Saber
-- @description: Every cut ticks, misses sting, combos build, finishing a map celebrates by accuracy. Needs the DataPuller mod.
-- @setup: Install "DataPuller" with your Beat Saber mod manager (BSManager / ModAssistant), then start any song.

-- BSDataPuller exposes two WebSockets: LiveData (score/combo/health, pushed
-- on every event) and MapData (in level / finished / failed).
sources = {
  { id = "live", url = "ws://127.0.0.1:2946/BSDataPuller/LiveData" },
  { id = "map", url = "ws://127.0.0.1:2946/BSDataPuller/MapData" },
}

local COMBO_MILESTONE = 25
local last_combo = 0
local last_misses = 0
local in_level = false
local celebrate_until = 0

local function num(x, fallback)
  if type(x) == "number" then return x end
  return fallback
end

function on_message(source, data)
  if source == "live" then
    if not in_level then return end
    local combo = num(data.Combo, last_combo)
    local misses = num(data.Misses, last_misses)

    if misses > last_misses then
      vibe.pulse(0.65, 0.3)                      -- missed / bad cut
    elseif combo > last_combo then
      if combo % COMBO_MILESTONE == 0 then
        vibe.pulse(0.45, 0.3)                    -- combo milestone
        vibe.status(combo .. " combo!")
      else
        vibe.pulse(0.18, 0.08)                   -- rhythm tick per cut
      end
    end
    last_combo, last_misses = combo, misses

    -- Danger hum when the energy bar runs low (PlayerHealth is 0–100).
    local health = num(data.PlayerHealth, 100)
    if health < 30 then
      vibe.set(0.12 + (1 - health / 30) * 0.1)
    else
      vibe.set(0)
    end
  elseif source == "map" then
    local was = in_level
    in_level = data.InLevel == true and data.LevelPaused ~= true

    if was and data.LevelFailed == true then
      vibe.pulse(0.95, 1.5)
      vibe.status("Level failed!")
    elseif was and data.LevelFinished == true then
      celebrate_until = vibe.now() + 4.0
      vibe.status("Level finished!")
    end
    if not in_level then
      last_combo, last_misses = 0, 0
      if vibe.now() >= celebrate_until then vibe.set(0) end
    end
  end
end

function on_tick(now)
  if now < celebrate_until then
    vibe.set(0.35 + 0.25 * math.sin((celebrate_until - now) * 4.0))
  elseif celebrate_until > 0 and now < celebrate_until + 0.2 then
    vibe.set(0)
    celebrate_until = 0
  end
end
