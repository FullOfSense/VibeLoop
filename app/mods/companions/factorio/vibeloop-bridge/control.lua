-- VibeLoop bridge: append one JSON line per interesting event to
-- script-output/vibeloop.jsonl. VibeLoop tails that file.
-- Works on Factorio 2.x (helpers.*) and 1.1 (game.*).

local function write_file(data, append)
  local w = (helpers and helpers.write_file) or game.write_file
  w("vibeloop.jsonl", data, append)
end

local function to_json(tbl)
  local j = (helpers and helpers.table_to_json) or game.table_to_json
  return j(tbl)
end

local function emit(tbl)
  write_file(to_json(tbl) .. "\n", true)
end

-- Fresh file per save load, so the log never grows unboundedly.
script.on_init(function()
  write_file("", false)
end)
script.on_load(function() end)

script.on_event(
  defines.events.on_entity_damaged,
  function(e)
    local ent = e.entity
    if ent and ent.valid then
      emit({
        e = "dmg",
        f = e.final_damage_amount / math.max(ent.max_health or 250, 1),
      })
    end
  end,
  { { filter = "type", type = "character" } }
)

script.on_event(defines.events.on_player_died, function()
  emit({ e = "died" })
end)

script.on_event(defines.events.on_research_finished, function(e)
  -- by_script covers console cheats; only real research celebrates.
  if not e.by_script then
    emit({ e = "research" })
  end
end)

script.on_event(defines.events.on_rocket_launched, function()
  emit({ e = "rocket" })
end)
