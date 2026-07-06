--- STEAMODDED HEADER
--- MOD_NAME: VibeLoop Bridge
--- MOD_ID: VibeLoopBridge
--- MOD_AUTHOR: [FullOfSense]
--- MOD_DESCRIPTION: Streams scoring events to vibeloop.jsonl for VibeLoop haptics.

-- Strategy: Balatro has no stable event API, but G.GAME/G.STATE are the
-- most stable internals there are. We piggyback on Game:update and emit a
-- JSON line whenever something observable changes. The file lives in the
-- LÖVE save directory (same place as the Mods folder).

local FILE = "vibeloop.jsonl"
local last = { chips = -1, dollars = nil, state = nil, ante = nil }

local function esc(s)
  return tostring(s):gsub('[\\"]', '\\%0'):gsub('\n', ' ')
end

local function emit(t)
  local parts = {}
  for k, v in pairs(t) do
    if type(v) == "number" then
      parts[#parts + 1] = string.format('"%s":%.4f', k, v)
    elseif type(v) == "boolean" then
      parts[#parts + 1] = string.format('"%s":%s', k, tostring(v))
    else
      parts[#parts + 1] = string.format('"%s":"%s"', k, esc(v))
    end
  end
  pcall(love.filesystem.append, FILE, "{" .. table.concat(parts, ",") .. "}\n")
end

-- Fresh file each launch.
pcall(love.filesystem.write, FILE, "")

local state_names = nil
local function state_name(s)
  if not state_names then
    state_names = {}
    if G and G.STATES then
      for name, value in pairs(G.STATES) do
        state_names[value] = name
      end
    end
  end
  return state_names[s] or tostring(s)
end

local orig_update = Game.update
function Game:update(dt)
  orig_update(self, dt)
  if not G or not G.GAME then return end
  local ok = pcall(function()
    -- Hand scoring: round chips climbing toward the blind requirement.
    local chips = tonumber(G.GAME.chips) or 0
    local target = (G.GAME.blind and tonumber(G.GAME.blind.chips)) or 0
    if chips > (last.chips or 0) and chips > 0 then
      emit({ e = "score", chips = chips, target = math.max(target, 1) })
    end
    last.chips = chips

    -- Money changes (jokers, interest, skips).
    local dollars = tonumber(G.GAME.dollars)
    if dollars and last.dollars and dollars ~= last.dollars then
      emit({ e = "money", d = dollars - last.dollars })
    end
    last.dollars = dollars

    -- Ante climbing.
    local ante = G.GAME.round_resets and tonumber(G.GAME.round_resets.ante)
    if ante and last.ante and ante > last.ante then
      emit({ e = "ante", n = ante })
    end
    last.ante = ante or last.ante

    -- State transitions: blind beaten, game over, new round…
    if G.STATE ~= last.state then
      last.state = G.STATE
      local boss = (G.GAME.blind and G.GAME.blind.boss) == true
      emit({ e = "state", s = state_name(G.STATE), boss = boss })
    end
  end)
  if not ok then return end
end
