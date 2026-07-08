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

-- ── Per-action hooks ─────────────────────────────────────────────────────
-- All defensive: only installed if the target exists, and the emit side is
-- wrapped so a bridge bug can never break the game.

-- Scoring pops: update_hand_text fires for every chip/mult change while a
-- hand is evaluated. Gate on HAND_PLAYED — the same function also animates
-- the score preview while you're still selecting cards.
if type(update_hand_text) == "function" then
  local orig_uht = update_hand_text
  function update_hand_text(config, vals)
    pcall(function()
      if G and G.STATES and G.STATE == G.STATES.HAND_PLAYED
        and type(vals) == "table" then
        local chips = tonumber(vals.chips)
        local mult = tonumber(vals.mult)
        if chips or mult then
          emit({ e = "pop", chips = chips or -1, mult = mult or -1 })
        end
      end
    end)
    return orig_uht(config, vals)
  end
end

-- Each card dealt into your hand.
if type(draw_card) == "function" then
  local orig_draw = draw_card
  function draw_card(from, to, ...)
    pcall(function()
      if G and to == G.hand then emit({ e = "draw" }) end
    end)
    return orig_draw(from, to, ...)
  end
end

-- Play / discard button presses, with how many cards were committed.
local function hook_action(name, event)
  if G and G.FUNCS and type(G.FUNCS[name]) == "function" then
    local orig = G.FUNCS[name]
    G.FUNCS[name] = function(...)
      pcall(function()
        local n = (G.hand and G.hand.highlighted and #G.hand.highlighted) or 0
        emit({ e = event, n = n })
      end)
      return orig(...)
    end
  end
end
hook_action("play_cards_from_highlighted", "play")
hook_action("discard_cards_from_highlighted", "discard")

-- Consumable use (tarot/planet/spectral) — costs no money, so the money
-- watcher below never sees it.
if G and G.FUNCS and type(G.FUNCS.use_card) == "function" then
  local orig_use = G.FUNCS.use_card
  G.FUNCS.use_card = function(...)
    pcall(function() emit({ e = "use" }) end)
    return orig_use(...)
  end
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
