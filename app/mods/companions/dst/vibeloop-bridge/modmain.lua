-- VibeLoop bridge for Don't Starve Together. Client-only mod: listens to
-- the local player's replicated events and prints marker lines, which land
-- in client_log.txt. VibeLoop tails that file.

local function announce(s)
  print("VIBELOOP " .. s)
end

AddPlayerPostInit(function(inst)
  inst:DoTaskInTime(0, function()
    -- Only ever watch the local player, not everyone who spawns.
    if GLOBAL.ThePlayer ~= inst then
      return
    end

    inst:ListenForEvent("healthdelta", function(_, data)
      if not data then return end
      local old = data.oldpercent or 0
      local new = data.newpercent or 0
      if new < old then
        announce(string.format("dmg %.3f", old - new))
      end
    end)

    inst:ListenForEvent("death", function()
      announce("died")
    end)

    inst:ListenForEvent("respawnfromghost", function()
      announce("respawn")
    end)

    inst:ListenForEvent("attacked", function()
      announce("hit")
    end)

    announce("ready")
  end)
end)
