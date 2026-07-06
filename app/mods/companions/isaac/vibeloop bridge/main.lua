-- VibeLoop bridge for The Binding of Isaac: Repentance / Repentance+.
-- Writes "VIBELOOP <event> …" lines into the game's log.txt via
-- Isaac.DebugString; VibeLoop tails the log and reacts.

local mod = RegisterMod("VibeLoop Bridge", 1)

local function announce(s)
  Isaac.DebugString("VIBELOOP " .. s)
end

-- Player took damage: report amount and current max (both in half-hearts).
mod:AddCallback(ModCallbacks.MC_ENTITY_TAKE_DMG, function(_, entity, amount)
  local player = entity:ToPlayer()
  if player then
    local max = math.max(player:GetMaxHearts() + player:GetSoulHearts(), 1)
    announce(string.format("dmg %.1f %d", amount, max))
  end
end, EntityType.ENTITY_PLAYER)

mod:AddCallback(ModCallbacks.MC_POST_ENTITY_KILL, function(_, entity)
  if entity:ToPlayer() then
    announce("died")
  elseif entity:ToNPC() and entity:IsBoss() then
    announce("boss")
  end
end)

mod:AddCallback(ModCallbacks.MC_POST_NEW_LEVEL, function()
  announce("level")
end)

mod:AddCallback(ModCallbacks.MC_POST_GAME_STARTED, function()
  announce("run")
end)
