-- @name: Test Pattern (no game needed)
-- @game: Demo
-- @description: Repeating 24 s pattern — pulses, ramp, spikes, wave, rest. Verifies toys and sessions without any game.
-- @setup: Nothing to run — press start and the pattern plays on its own.

vibe.log("Demo pattern loaded — 24 s loop: pulses, ramp, spikes, wave, rest")

local CYCLE = 24.0
local last_phase = ""
local last_slot = -1

local function phase(name)
  if name ~= last_phase then
    last_phase = name
    vibe.status("Test pattern: " .. name)
    vibe.log("Phase: " .. name)
  end
end

-- Fire one pulse each time `now` crosses into a new `interval`-sized slot.
local function every(now, interval, level, seconds)
  local slot = math.floor(now / interval)
  if slot ~= last_slot then
    last_slot = slot
    vibe.pulse(level, seconds)
  end
end

function on_tick(now)
  local t = now % CYCLE

  if t < 6.0 then
    phase("steady pulses at 40%")
    every(now, 1.2, 0.40, 0.20)
  elseif t < 12.0 then
    phase("slow ramp up to 80%")
    vibe.set(0.80 * (t - 6.0) / 6.0)
  elseif t < 16.0 then
    phase("hard spikes at 90%")
    vibe.set(0)
    every(now, 1.0, 0.90, 0.25)
  elseif t < 21.0 then
    phase("smooth wave")
    vibe.set(0.40 + 0.35 * math.sin((t - 16.0) * 2.0 * math.pi / 2.5))
  else
    phase("rest — everything should stop")
    vibe.set(0)
  end
end
