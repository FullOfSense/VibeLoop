// VibeLoop frontend. No framework, no build step: talks to the Rust core
// through Tauri commands and events only.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);

const el = {
  toyDot: $("toy-dot"),
  toySummary: $("toy-summary"),
  deviceList: $("device-list"),
  test: $("btn-test"),
  rescan: $("btn-rescan"),
  tabs: $("mode-tabs"),
  modeExplain: $("mode-explain"),
  fieldRoom: $("field-room"),
  roomLabel: $("room-label"),
  room: $("room"),
  fieldPassword: $("field-password"),
  passwordLabel: $("password-label"),
  password: $("password"),
  eyePassword: $("eye-password"),
  fieldMod: $("field-mod"),
  mod: $("mod"),
  modDesc: $("mod-desc"),
  modSetup: $("mod-setup"),
  modsFolder: $("btn-mods-folder"),
  start: $("btn-start"),
  setup: $("setup"),
  running: $("running"),
  modeLabel: $("mode-label"),
  viewers: $("viewers"),
  shareCard: $("share-card"),
  shareRoom: $("share-room"),
  sharePwRow: $("share-pw-row"),
  sharePassword: $("share-password"),
  eyeShare: $("eye-share"),
  copy: $("btn-copy"),
  meterFill: $("meter-fill"),
  meterValue: $("meter-value"),
  status: $("status"),
  stop: $("btn-stop"),
  log: $("log"),
  engineLabel: $("engine-label"),
};

let mods = [];
let mode = "solo"; // solo | host | join
let sharePwVisible = false;
let activePassword = "";

const MODES = {
  solo: {
    explain: "Feel your own game. Nothing is shared — just you and your toy.",
    startLabel: "▶ START",
    runningLabel: "SOLO",
  },
  host: {
    explain:
      "Pick a room name, start, and share it with your viewers. Everyone in your room feels your game, live.",
    startLabel: "▶ START HOSTING",
    runningLabel: "HOSTING",
  },
  join: {
    explain:
      "Enter the room name your host shared (and the password, if they set one). You'll feel what they feel.",
    startLabel: "▶ JOIN ROOM",
    runningLabel: "VIEWING",
  },
};

// ── UI helpers ───────────────────────────────────────────────────────────────

function log(line) {
  const time = new Date().toLocaleTimeString();
  el.log.textContent += `[${time}] ${line}\n`;
  el.log.scrollTop = el.log.scrollHeight;
}

function setStatus(text) {
  el.status.textContent = text;
}

function renderMode() {
  for (const tab of el.tabs.children) {
    tab.classList.toggle("active", tab.dataset.mode === mode);
  }
  el.modeExplain.textContent = MODES[mode].explain;
  el.start.textContent = MODES[mode].startLabel;

  el.fieldRoom.classList.toggle("hidden", mode === "solo");
  el.fieldPassword.classList.toggle("hidden", mode === "solo");
  el.fieldMod.classList.toggle("hidden", mode === "join");

  if (mode === "host") {
    el.roomLabel.textContent = "YOUR ROOM NAME — viewers use it to find you";
    el.room.placeholder = "e.g. your streamer name";
    el.passwordLabel.textContent = "ROOM PASSWORD — optional, makes your room private";
    el.password.placeholder = "leave empty for a public room";
  } else if (mode === "join") {
    el.roomLabel.textContent = "HOST'S ROOM NAME — the name they shared";
    el.room.placeholder = "e.g. their streamer name";
    el.passwordLabel.textContent = "HOST'S PASSWORD — leave empty if they didn't set one";
    el.password.placeholder = "only needed for private rooms";
  }
}

function showRunning() {
  el.setup.classList.add("hidden");
  el.running.classList.remove("hidden");
  el.modeLabel.textContent = MODES[mode].runningLabel;
  el.viewers.classList.toggle("hidden", mode !== "host");

  const hosting = mode === "host";
  el.shareCard.classList.toggle("hidden", !hosting);
  if (hosting) {
    el.shareRoom.textContent = el.room.value.trim();
    activePassword = el.password.value;
    el.sharePwRow.classList.toggle("hidden", activePassword === "");
    sharePwVisible = false;
    renderSharePassword();
  }
}

function showSetup() {
  el.running.classList.add("hidden");
  el.setup.classList.remove("hidden");
  el.meterFill.style.width = "0%";
  el.meterValue.textContent = "0%";
  activePassword = "";
}

function renderSharePassword() {
  el.sharePassword.textContent = sharePwVisible
    ? activePassword
    : "•".repeat(Math.max(activePassword.length, 6));
}

function renderDevices(devices) {
  el.deviceList.innerHTML = "";
  if (!devices || devices.length === 0) {
    el.toyDot.className = "dot";
    el.toySummary.textContent = "No toys yet — turn your toy on, it connects automatically.";
    return;
  }
  el.toyDot.className = "dot ok";
  el.toySummary.textContent = `${devices.length} toy${devices.length > 1 ? "s" : ""} connected`;
  for (const d of devices) {
    const chip = document.createElement("span");
    chip.className = "chip" + (d.can_vibrate ? " vibrates" : "");
    chip.textContent = d.name;
    el.deviceList.appendChild(chip);
  }
}

function renderMods() {
  el.mod.innerHTML = "";
  if (mods.length === 0) {
    const opt = document.createElement("option");
    opt.textContent = "No mods found — put a .lua file in the mods folder";
    opt.value = "";
    el.mod.appendChild(opt);
    el.modDesc.textContent = "";
    el.modSetup.textContent = "";
    return;
  }
  for (const m of mods) {
    const opt = document.createElement("option");
    opt.value = m.id;
    opt.textContent = `${m.game} · ${m.name}`;
    el.mod.appendChild(opt);
  }
  updateModDesc();
}

function updateModDesc() {
  const m = mods.find((x) => x.id === el.mod.value);
  el.modDesc.textContent = m ? m.description : "";
  el.modSetup.textContent = m && m.setup ? `⚙ ${m.setup}` : "";
}

function toast(message) {
  setStatus(message);
  log(message);
}

// ── Actions ──────────────────────────────────────────────────────────────────

el.tabs.addEventListener("click", (e) => {
  const tab = e.target.closest(".tab");
  if (!tab) return;
  mode = tab.dataset.mode;
  renderMode();
});

async function guarded(action, button) {
  button.disabled = true;
  try {
    await action();
  } catch (e) {
    toast(String(e));
    showSetup();
  } finally {
    button.disabled = false;
  }
}

el.start.addEventListener("click", () =>
  guarded(async () => {
    if (mode === "solo") {
      if (!el.mod.value) throw "Pick a game mod first.";
      await invoke("start_solo", { modId: el.mod.value });
      showRunning();
      setStatus("Running solo — start your game.");
    } else if (mode === "host") {
      if (!el.mod.value) throw "Pick a game mod first.";
      await invoke("start_host", {
        username: el.room.value,
        password: el.password.value,
        modId: el.mod.value,
      });
      showRunning();
      setStatus("Hosting — start your game. Viewers can join any time.");
    } else {
      await invoke("start_join", {
        username: el.room.value,
        password: el.password.value,
      });
      showRunning();
      setStatus("Looking for the host…");
    }
  }, el.start)
);

el.stop.addEventListener("click", () =>
  guarded(async () => {
    await invoke("stop");
    showSetup();
  }, el.stop)
);

el.test.addEventListener("click", () =>
  guarded(async () => {
    await invoke("test_buzz");
    log("Test buzz sent.");
  }, el.test)
);

el.rescan.addEventListener("click", () =>
  guarded(async () => {
    await invoke("scan_devices");
    log("Scanning for toys…");
  }, el.rescan)
);

el.modsFolder.addEventListener("click", () =>
  guarded(async () => {
    const dir = await invoke("open_mods_folder");
    log(`Mods folder: ${dir}`);
    mods = await invoke("list_mods");
    renderMods();
  }, el.modsFolder)
);

el.mod.addEventListener("change", updateModDesc);

// Password visibility toggles.
el.eyePassword.addEventListener("click", () => {
  el.password.type = el.password.type === "password" ? "text" : "password";
  el.eyePassword.classList.toggle("on", el.password.type === "text");
});
el.eyeShare.addEventListener("click", () => {
  sharePwVisible = !sharePwVisible;
  el.eyeShare.classList.toggle("on", sharePwVisible);
  renderSharePassword();
});

// Copy room details for stream chat.
el.copy.addEventListener("click", async () => {
  const room = el.shareRoom.textContent;
  const text = activePassword
    ? `Feel my game with VibeLoop! Room: ${room} · Password: ${activePassword}`
    : `Feel my game with VibeLoop! Room: ${room}`;
  try {
    await navigator.clipboard.writeText(text);
    el.copy.textContent = "Copied!";
  } catch {
    // Clipboard API unavailable — show the text so it can be copied manually.
    log(`Copy this: ${text}`);
    el.copy.textContent = "See log ↓";
  }
  setTimeout(() => (el.copy.textContent = "Copy details"), 2000);
});

// Remember the room name between launches.
el.room.value = localStorage.getItem("vibeloop.username") || "";
el.room.addEventListener("change", () =>
  localStorage.setItem("vibeloop.username", el.room.value.trim())
);

// ── Events from the core ─────────────────────────────────────────────────────

listen("log", (e) => log(e.payload));
listen("status", (e) => setStatus(e.payload));
listen("devices", (e) => renderDevices(e.payload));
listen("viewers", (e) => {
  const n = e.payload;
  el.viewers.textContent = `● ${n} viewer${n === 1 ? "" : "s"}`;
});
listen("intensity", (e) => {
  const pct = Math.round(e.payload * 100);
  el.meterFill.style.width = `${pct}%`;
  el.meterValue.textContent = `${pct}%`;
});
listen("session-ended", async () => {
  await invoke("stop").catch(() => {});
  showSetup();
});
listen("snapshot", (e) => {
  const s = e.payload;
  el.engineLabel.textContent = s.engine ? `toy engine: ${s.engine}` : "";
  renderDevices(s.devices);
});

// ── Init ─────────────────────────────────────────────────────────────────────

(async function init() {
  renderMode();
  try {
    mods = await invoke("list_mods");
    renderMods();
    await invoke("get_snapshot");
  } catch (e) {
    log(`Init error: ${e}`);
  }
  log("VibeLoop ready.");
})();
