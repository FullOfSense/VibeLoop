-- @name: Team Fortress 2 — Balanced
-- @game: Team Fortress 2
-- @description: Your kills reward (crits extra), your deaths sting. Reads the kill feed from TF2's console log.
-- @setup: Add "-condebug -conclearlog" to TF2's Steam launch options and set MY_NICK inside this file to your exact in-game name.

-- TF2 writes its console (including the kill feed) to tf/console.log with
-- the -condebug launch option. No mod, no VAC concerns.
sources = {
  {
    id = "log",
    type = "file",
    paths = {
      "~/.steam/steam/steamapps/common/Team Fortress 2/tf/console.log",
      "~/.local/share/Steam/steamapps/common/Team Fortress 2/tf/console.log",
      "${ProgramFiles(x86)}/Steam/steamapps/common/Team Fortress 2/tf/console.log",
      "C:/Program Files (x86)/Steam/steamapps/common/Team Fortress 2/tf/console.log",
      "~/Library/Application Support/Steam/steamapps/common/Team Fortress 2/tf/console.log",
    },
  },
}

-- REQUIRED: your exact in-game name. The kill feed is plain text — this is
-- the only way to know which kills are yours.
local MY_NICK = ""

local warned = false

function on_message(source, data)
  local line = data.line
  if type(line) ~= "string" then return end

  if MY_NICK == "" then
    if not warned then
      warned = true
      vibe.status("Set MY_NICK in team_fortress2.lua to feel your kills!")
    end
    return
  end

  -- Kill feed lines: "Attacker killed Victim with weapon." + " (crit)"
  local attacker, victim = line:match("^(.-) killed (.-) with ")
  if attacker then
    local crit = line:find("%(crit%)") ~= nil
    if attacker == MY_NICK then
      vibe.pulse(crit and 0.75 or 0.55, crit and 0.5 or 0.35)
      vibe.status(crit and "CRIT kill!" or "Kill!")
    elseif victim == MY_NICK then
      vibe.pulse(0.85, 0.8)
      vibe.status("You died")
    end
    return
  end

  if line == MY_NICK .. " suicided." then
    vibe.pulse(0.7, 0.6)
    vibe.status("Ouch.")
  end
end
