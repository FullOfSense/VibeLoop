-- @name: VRChat — Avatar Contact
-- @game: VRChat
-- @description: Drives intensity from an avatar parameter (e.g. a Contact Receiver in proximity mode) over OSC.
-- @setup: In VRChat enable OSC (Action Menu → Options → OSC → Enabled). Your avatar needs a float parameter named VibeLoop — a VRC Contact Receiver in Proximity mode works perfectly.

-- VRChat broadcasts every avatar parameter change as OSC on UDP port 9001.
-- If another OSC app (VRCFaceTracking etc.) already uses 9001, run an OSC
-- router and point one output here — see the modding guide.
sources = {
  { id = "vrc", type = "osc", port = 9001 },
}

-- ── Tune these ──────────────────────────────────────────────────────────
-- Parameter to follow. A Contact Receiver in "Proximity" mode outputs 0→1
-- as the contact approaches, which maps directly onto intensity.
local PARAMETER = "VibeLoop"
local SCALE = 1.0            -- multiply the parameter before sending it out
-- ────────────────────────────────────────────────────────────────────────

local ADDRESS = "/avatar/parameters/" .. PARAMETER
local last_value = 0
local last_update = -10

function on_message(source, data)
  if data.addr == ADDRESS then
    local v = data.args and data.args[1]
    if type(v) == "number" then
      last_value = math.max(0, math.min(v * SCALE, 1))
      last_update = vibe.now()
      vibe.set(last_value)
    elseif type(v) == "boolean" then
      -- Bool parameters work too: on/off at 60%.
      last_value = v and 0.6 or 0
      last_update = vibe.now()
      vibe.set(last_value)
    end
  elseif data.addr == "/avatar/change" then
    -- New avatar: release everything until its parameters speak up.
    last_value = 0
    vibe.set(0)
    vibe.status("Avatar changed — waiting for " .. PARAMETER)
  end
end

function on_tick(now)
  -- OSC is UDP: if the final "release" packet is lost, don't stay buzzing.
  -- While touched, VRChat streams updates constantly, so 1.5 s of silence
  -- with a non-zero value means the contact actually ended.
  if last_value > 0 and now - last_update > 1.5 then
    last_value = 0
    vibe.set(0)
  end
end
