-- @name: Don't Starve Together — Balanced
-- @game: Don't Starve Together
-- @description: Getting hit buzzes, health loss scales it, death stings, respawning resets. Client-side only — works on any server.
-- @setup: Install the vibeloop-bridge client mod from mods/companions/dst/ and enable it under Mods → Client Mods.

-- The bridge prints "VIBELOOP …" markers, which DST writes into
-- client_log.txt along with everything else.
sources = {
  {
    id = "log",
    type = "file",
    paths = {
      "~/.klei/DoNotStarveTogether/client_log.txt",
      "${USERPROFILE}/Documents/Klei/DoNotStarveTogether/client_log.txt",
      "~/Documents/Klei/DoNotStarveTogether/client_log.txt",
    },
  },
}

local function num(x, fallback)
  local n = tonumber(x)
  if n then return n end
  return fallback
end

function on_message(source, data)
  local line = data.line
  if type(line) ~= "string" then return end
  local event, rest = line:match("VIBELOOP (%S+)%s*(.*)")
  if not event then return end

  if event == "dmg" then
    -- rest = fraction of max health lost (0–1).
    local f = num(rest, 0.05)
    vibe.pulse(math.min(0.2 + f * 2.0, 0.85), 0.35)
  elseif event == "hit" then
    -- "attacked" fires even for blocked/armored hits — keep it light.
    vibe.pulse(0.25, 0.15)
  elseif event == "died" then
    vibe.pulse(0.95, 1.5)
    vibe.status("You died!")
  elseif event == "respawn" then
    vibe.set(0)
    vibe.status("Back among the living")
  elseif event == "ready" then
    vibe.status("Bridge connected — have fun!")
  end
end
