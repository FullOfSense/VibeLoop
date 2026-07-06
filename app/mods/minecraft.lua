-- @name: Minecraft — Balanced
-- @game: Minecraft
-- @description: Taking damage buzzes by hearts lost, dying stings, respawning resets, level-ups reward. Java Edition with Fabric.
-- @setup: Install Fabric + the vibeloop-bridge jar from mods/companions/minecraft/ into your .minecraft/mods folder (needs Fabric API).

-- The bridge writes to ~/.vibeloop/minecraft.jsonl — the same place on
-- every launcher (vanilla, Prism, CurseForge) and every OS.
sources = {
  {
    id = "log",
    type = "file",
    paths = {
      "~/.vibeloop/minecraft.jsonl",
      "${USERPROFILE}/.vibeloop/minecraft.jsonl",
    },
  },
}

local function num(x, fallback)
  if type(x) == "number" then return x end
  return fallback
end

function on_message(source, data)
  if data.e == "dmg" then
    -- f = fraction of max health lost (half a heart on 20 HP ≈ 0.025).
    local f = num(data.f, 0)
    vibe.pulse(math.min(0.2 + f * 1.8, 0.9), 0.3)
  elseif data.e == "died" then
    vibe.pulse(0.95, 1.5)
    vibe.status("You died!")
  elseif data.e == "respawn" then
    vibe.set(0)
    vibe.status("Respawned")
  elseif data.e == "levelup" then
    vibe.pulse(0.4, 0.4)
    vibe.status("Level " .. num(data.n, 0))
  end
end
