-- @name: R.E.P.O. — Balanced
-- @game: R.E.P.O.
-- @description: Taking hits buzzes by damage, dying stings, heals tickle, completed extractions celebrate. Local player only.
-- @setup: Install BepInEx (BepInExPack from Thunderstore) and drop VibeLoopBridge.REPO.dll from mods/companions/repo/ into BepInEx/plugins.

-- The bridge writes to <home>/.vibeloop/repo.jsonl. Under Proton on Linux,
-- "home" is inside the game's prefix (compatdata/3241660).
sources = {
  {
    id = "log",
    type = "file",
    paths = {
      "${USERPROFILE}/.vibeloop/repo.jsonl",
      "~/.steam/steam/steamapps/compatdata/3241660/pfx/drive_c/users/steamuser/.vibeloop/repo.jsonl",
      "~/.local/share/Steam/steamapps/compatdata/3241660/pfx/drive_c/users/steamuser/.vibeloop/repo.jsonl",
      "/media/patrixonix/HDD/SteamLibrary/steamapps/compatdata/3241660/pfx/drive_c/users/steamuser/.vibeloop/repo.jsonl",
    },
  },
}

local celebrate_until = 0

local function num(x, fallback)
  if type(x) == "number" then return x end
  return fallback
end

function on_message(source, data)
  if data.e == "dmg" then
    local f = num(data.f, 0)
    vibe.pulse(math.min(0.25 + f * 1.5, 0.9), 0.35)
  elseif data.e == "died" then
    vibe.pulse(0.95, 1.5)
    vibe.status("You died!")
  elseif data.e == "heal" then
    vibe.pulse(0.2, 0.15)
  elseif data.e == "extracted" then
    celebrate_until = vibe.now() + 5.0
    vibe.status("Extraction complete!")
  end
end

function on_tick(now)
  if now < celebrate_until then
    vibe.set(0.3 + 0.25 * math.sin((celebrate_until - now) * 3.5))
  elseif celebrate_until > 0 and now < celebrate_until + 0.2 then
    vibe.set(0)
    celebrate_until = 0
  end
end
