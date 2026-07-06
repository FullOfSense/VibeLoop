-- @name: The Binding of Isaac — Balanced
-- @game: The Binding of Isaac
-- @description: Taking hearts of damage buzzes by how much you lost, deaths sting, boss kills and new floors reward.
-- @setup: Install the "vibeloop bridge" mod from mods/companions/isaac/ into Isaac's mods folder and enable it in-game.

-- The bridge writes "VIBELOOP …" markers into the game's own log.txt.
sources = {
  {
    id = "log",
    type = "file",
    paths = {
      "${USERPROFILE}/Documents/My Games/Binding of Isaac Repentance+/log.txt",
      "${USERPROFILE}/Documents/My Games/Binding of Isaac Repentance/log.txt",
      "~/.local/share/binding of isaac repentance+/log.txt",
      "~/.local/share/binding of isaac repentance/log.txt",
      "~/Library/Application Support/Binding of Isaac Repentance+/log.txt",
      "~/Library/Application Support/Binding of Isaac Repentance/log.txt",
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
  -- Log lines look like: [INFO] - Lua Debug: VIBELOOP dmg 1.0 6
  local event, rest = line:match("VIBELOOP (%S+)%s*(.*)")
  if not event then return end

  if event == "dmg" then
    -- amount and max hearts, both in half-hearts.
    local amount, max = rest:match("(%S+)%s+(%S+)")
    local f = num(amount, 1) / math.max(num(max, 6), 1)
    vibe.pulse(math.min(0.25 + f * 1.6, 0.9), 0.35)
  elseif event == "died" then
    vibe.pulse(0.95, 1.5)
    vibe.status("You died…")
  elseif event == "boss" then
    vibe.pulse(0.7, 0.8)
    vibe.status("Boss down!")
  elseif event == "level" then
    vibe.pulse(0.35, 0.4)
    vibe.status("New floor")
  elseif event == "run" then
    vibe.set(0)
    vibe.status("New run — good luck!")
  end
end
