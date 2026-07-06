-- @name: WEBFISHING — Balanced
-- @game: WEBFISHING
-- @description: You FEEL the bite the moment a fish strikes, landing it celebrates, rod level-ups reward.
-- @setup: Install GDWeave, then copy the FullOfSense.VibeLoop folder from mods/companions/webfishing/ into WEBFISHING/GDWeave/Mods/.

-- The bridge writes to Godot's user dir for this game.
sources = {
  {
    id = "log",
    type = "file",
    paths = {
      "${APPDATA}/Godot/app_userdata/webfishing_2_newver/vibeloop.jsonl",
      -- Proton on Linux: found in whichever Steam library holds the game.
      "${STEAM_LIBRARIES}/compatdata/3146520/pfx/drive_c/users/steamuser/AppData/Roaming/Godot/app_userdata/webfishing_2_newver/vibeloop.jsonl",
    },
  },
}

local function num(x, fallback)
  if type(x) == "number" then return x end
  return fallback
end

function on_message(source, data)
  if data.e == "bite" then
    -- The moment the fish strikes — the whole point of this mod.
    vibe.pulse(0.8, 0.45)
    vibe.status("Bite!!")
  elseif data.e == "catch" then
    vibe.pulse(0.55, 0.7)
    vibe.status("Fish landed!")
  elseif data.e == "levelup" then
    vibe.pulse(0.45, 0.5)
    vibe.status("Rod level " .. num(data.n, 0) .. "!")
  end
end
