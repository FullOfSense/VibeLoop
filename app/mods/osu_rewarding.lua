-- @name: osu! — Rewarding
-- @game: osu!
-- @description: Buzz on every hit — stronger for better judgements. Miss and fail hit hardest, passing a map earns a celebration pattern scaled to accuracy.
-- @setup: Run tosu (github.com/tosuapp/tosu) alongside osu! — works with stable and lazer.
--
-- Works with osu! stable AND osu!lazer through tosu (https://github.com/tosuapp/tosu).
-- Just start tosu, start osu!, play. VibeLoop reconnects automatically.

sources = {
  { id = "v1", url = "ws://127.0.0.1:24050/ws" },
  { id = "v2", url = "ws://127.0.0.1:24050/websocket/v2" },
}

-- ── Tuning ───────────────────────────────────────────────────────────────────
local LEVEL = { meh = 0.07, ok = 0.15, great = 0.30, perfect = 0.50, miss = 0.75, fail = 1.0 }
local DUR   = { meh = 0.07, ok = 0.09, great = 0.11, perfect = 0.14, miss = 0.20 }
local FAIL_DURATION = 3.0
local WIN_DURATION  = 4.0

-- ── State ────────────────────────────────────────────────────────────────────
local st = {
  status = 0, h300 = 0, h100 = 0, h50 = 0, miss = 0,
  geki = 0, katu = 0, accuracy = 0, failed = false,
}
local win = { until_t = 0, next_burst = 0, burst_on = false, intensity = 1.0 }

-- ── Game events (tosu v1: hits, status, accuracy) ────────────────────────────
local function handle_v1(data)
  local gp   = data.gameplay or {}
  local menu = data.menu or {}
  local hits = gp.hits or {}

  local prev = {
    status = st.status, h300 = st.h300, h100 = st.h100, h50 = st.h50,
    miss = st.miss, geki = st.geki, katu = st.katu,
  }

  st.status   = menu.state or 0
  st.h300     = hits["300"] or 0
  st.h100     = hits["100"] or 0
  st.h50      = hits["50"] or 0
  st.miss     = hits["0"] or 0
  st.geki     = hits.geki or 0
  st.katu     = hits.katu or 0
  st.accuracy = gp.accuracy or 0

  local now = vibe.now()

  -- Map passed: state 2 (playing) → 7 (results), not failed.
  if prev.status == 2 and st.status == 7 and not st.failed then
    win.intensity  = math.max(0.20, st.accuracy / 100.0)
    win.until_t    = now + WIN_DURATION
    win.next_burst = now
    win.burst_on   = false
    vibe.log(string.format("Map passed! %.1f%% accuracy", st.accuracy))
    return
  end

  -- Leaving gameplay for any other reason: nothing to do.
  if st.status ~= 2 then
    st.failed = false
    return
  end

  -- Judgement pulses, checked worst-first so one frame = one pulse.
  if st.miss > prev.miss then
    vibe.pulse(LEVEL.miss, DUR.miss)
  elseif st.geki > prev.geki then
    vibe.pulse(LEVEL.perfect, DUR.perfect)
  elseif (st.h300 - prev.h300) - (st.geki - prev.geki) > 0 then
    vibe.pulse(LEVEL.great, DUR.great)
  elseif (st.h100 - prev.h100) - (st.katu - prev.katu) > 0 then
    vibe.pulse(LEVEL.ok, DUR.ok)
  elseif st.h50 > prev.h50 then
    vibe.pulse(LEVEL.meh, DUR.meh)
  end
end

-- ── Fail detection (tosu v2: play.failed flip) ───────────────────────────────
local function handle_v2(data)
  local play = data.play or {}
  local failed_now = play.failed or false
  if failed_now and not st.failed then
    vibe.pulse(LEVEL.fail, FAIL_DURATION)
    vibe.log("Map failed!")
  end
  st.failed = failed_now
end

function on_message(source, data)
  if source == "v1" then
    handle_v1(data)
  elseif source == "v2" then
    handle_v2(data)
  end
end

-- ── Win celebration: random on/off bursts scaled to accuracy ─────────────────
function on_tick(now)
  if now < win.until_t then
    if now >= win.next_burst then
      win.burst_on = not win.burst_on
      win.next_burst = now + (win.burst_on and (0.1 + math.random() * 0.3)
                                            or (0.1 + math.random() * 0.2))
    end
    vibe.set(win.burst_on and win.intensity or 0)
  elseif win.until_t > 0 then
    win.until_t = 0
    vibe.set(0)
  end
end
