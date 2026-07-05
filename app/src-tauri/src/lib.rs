//! Tauri shell: owns app state, exposes commands to the frontend, and forwards
//! every core event to the UI as human-readable messages. All heavy lifting
//! lives in vibeloop-core.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use tokio::sync::{mpsc, Mutex};

use vibeloop_core::device::{DeviceInfo, DeviceManager, INTIFACE_CENTRAL_URL};
use vibeloop_core::modengine::{self, ModEvent, ModInfo, RunningMod};
use vibeloop_core::session::{self, Session, SessionEvent};
use vibeloop_core::IntensityBus;

#[cfg(feature = "engine")]
use vibeloop_core::engine::EmbeddedEngine;

// ─── State ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "lowercase", tag = "kind")]
pub enum Mode {
    #[default]
    Idle,
    Solo {
        mod_id: String,
    },
    Host {
        room: String,
        mod_id: String,
    },
    Viewer {
        room: String,
    },
}

#[derive(Default)]
struct Inner {
    devices: Option<DeviceManager>,
    #[cfg(feature = "engine")]
    engine: Option<EmbeddedEngine>,
    engine_label: String,
    session: Option<Session>,
    running_mod: Option<RunningMod>,
    mode: Mode,
    viewers: usize,
}

struct AppState {
    bus: IntensityBus,
    inner: Arc<Mutex<Inner>>,
}

// ─── UI event helpers ────────────────────────────────────────────────────────

#[derive(Clone, Serialize)]
struct Snapshot {
    mode: Mode,
    viewers: usize,
    devices: Vec<DeviceInfo>,
    engine: String,
    #[serde(rename = "engineReady")]
    engine_ready: bool,
}

fn emit_log(app: &AppHandle, line: impl Into<String>) {
    let _ = app.emit("log", line.into());
}

fn emit_status(app: &AppHandle, line: impl Into<String>) {
    let _ = app.emit("status", line.into());
}

async fn emit_snapshot(app: &AppHandle, state: &AppState) {
    let inner = state.inner.lock().await;
    let snapshot = Snapshot {
        mode: inner.mode.clone(),
        viewers: inner.viewers,
        devices: inner
            .devices
            .as_ref()
            .map(|d| d.devices())
            .unwrap_or_default(),
        engine: inner.engine_label.clone(),
        engine_ready: inner.devices.as_ref().map(|d| d.connected()).unwrap_or(false),
    };
    let _ = app.emit("snapshot", snapshot);
}

// ─── Device / engine bring-up ────────────────────────────────────────────────

/// Connects toys: prefer a running Intiface Central, otherwise start the
/// embedded engine. Retries forever in the background until something works,
/// so plugging things in later "just works".
async fn device_bringup(app: AppHandle, state: Arc<Mutex<Inner>>, bus: IntensityBus) {
    let (dev_tx, mut dev_rx) = mpsc::unbounded_channel();

    // Pump device events → UI.
    {
        let app = app.clone();
        let state = state.clone();
        tokio::spawn(async move {
            while let Some(event) = dev_rx.recv().await {
                use vibeloop_core::device::DeviceEvent::*;
                match event {
                    Connected { server } => {
                        emit_log(&app, format!("Toy engine ready ({server})"))
                    }
                    Disconnected => {
                        emit_log(&app, "Toy engine disconnected — reconnecting…");
                        // Drop the dead manager; the bring-up loop below restarts.
                        let mut inner = state.lock().await;
                        inner.devices = None;
                    }
                    DevicesChanged(devs) => {
                        if devs.is_empty() {
                            emit_status(&app, "No toys found yet — turn your toy on, it will connect automatically.");
                        } else {
                            let names: Vec<_> = devs.iter().map(|d| d.name.clone()).collect();
                            emit_status(&app, format!("Connected toys: {}", names.join(", ")));
                        }
                        let _ = app.emit("devices", devs);
                    }
                    Log(line) => emit_log(&app, line),
                }
            }
        });
    }

    loop {
        // Already connected? Just keep watch.
        {
            let inner = state.lock().await;
            if inner
                .devices
                .as_ref()
                .map(|d| d.connected())
                .unwrap_or(false)
            {
                drop(inner);
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                continue;
            }
        }

        // 1) Existing Intiface Central?
        let external = DeviceManager::connect(
            INTIFACE_CENTRAL_URL,
            bus.subscribe(),
            dev_tx.clone(),
        )
        .await;

        let (manager, label) = match external {
            Ok(m) => (Some(m), "Intiface Central".to_string()),
            Err(_) => {
                // 2) Our own embedded engine.
                #[cfg(feature = "engine")]
                {
                    let need_engine = {
                        let inner = state.lock().await;
                        inner.engine.is_none()
                    };
                    if need_engine {
                        let (err_tx, mut err_rx) = mpsc::unbounded_channel::<String>();
                        {
                            let app = app.clone();
                            tokio::spawn(async move {
                                while let Some(e) = err_rx.recv().await {
                                    emit_log(&app, e);
                                }
                            });
                        }
                        match EmbeddedEngine::start(err_tx).await {
                            Ok(engine) => {
                                let mut inner = state.lock().await;
                                inner.engine = Some(engine);
                                drop(inner);
                                emit_log(&app, "Started built-in toy engine");
                                // Give it a moment to open its socket.
                                tokio::time::sleep(std::time::Duration::from_millis(750))
                                    .await;
                            }
                            Err(e) => {
                                emit_log(&app, format!("Could not start built-in toy engine: {e}"));
                            }
                        }
                    }
                    let url = format!(
                        "ws://127.0.0.1:{}",
                        vibeloop_core::device::EMBEDDED_ENGINE_PORT
                    );
                    match DeviceManager::connect(&url, bus.subscribe(), dev_tx.clone()).await
                    {
                        Ok(m) => (Some(m), "built-in engine".to_string()),
                        Err(_) => (None, String::new()),
                    }
                }
                #[cfg(not(feature = "engine"))]
                {
                    (None, String::new())
                }
            }
        };

        if let Some(manager) = manager {
            let _ = manager.scan().await;
            let mut inner = state.lock().await;
            inner.engine_label = label;
            inner.devices = Some(manager);
            drop(inner);
            emit_log(&app, "Scanning for toys… turn your toy on now.");
        } else {
            emit_status(
                &app,
                "Setting up toy engine… (if this persists: is Bluetooth enabled?)",
            );
        }

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

// ─── Mods ────────────────────────────────────────────────────────────────────

fn mods_dirs(app: &AppHandle) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(dir) = std::env::var("VIBELOOP_MODS_DIR") {
        dirs.push(PathBuf::from(dir));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("mods"));
        }
    }
    if let Ok(resources) = app.path().resource_dir() {
        dirs.push(resources.join("mods"));
    }
    if let Ok(data) = app.path().app_data_dir() {
        dirs.push(data.join("mods"));
    }
    dirs
}

fn collect_mods(app: &AppHandle) -> Vec<ModInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut all = Vec::new();
    for dir in mods_dirs(app) {
        for info in modengine::scan_mods(&dir) {
            if seen.insert(info.id.clone()) {
                all.push(info);
            }
        }
    }
    all
}

async fn start_mod_for(app: &AppHandle, state: &AppState, mod_id: &str) -> Result<(), String> {
    let info = collect_mods(app)
        .into_iter()
        .find(|m| m.id == mod_id)
        .ok_or_else(|| format!("Mod '{mod_id}' not found. Is the file still in the mods folder?"))?;

    let (mod_tx, mut mod_rx) = mpsc::unbounded_channel();
    {
        let app = app.clone();
        tokio::spawn(async move {
            while let Some(event) = mod_rx.recv().await {
                match event {
                    ModEvent::SourceUp { source } => {
                        emit_status(&app, format!("Game link '{source}' connected — play!"));
                    }
                    ModEvent::SourceDown { source, detail } => {
                        emit_status(&app, format!("Waiting for game link '{source}': {detail}"));
                    }
                    ModEvent::Status(s) => emit_status(&app, s),
                    ModEvent::Log(l) => emit_log(&app, l),
                    ModEvent::Failed(e) => {
                        emit_log(&app, e.clone());
                        emit_status(&app, e);
                    }
                }
            }
        });
    }

    let running = modengine::run_mod(&info.path, &state.bus, mod_tx)
        .await
        .map_err(|e| format!("{e:#}"))?;
    state.inner.lock().await.running_mod = Some(running);
    emit_log(app, format!("Mod loaded: {}", info.name));
    Ok(())
}

// ─── Session event pump ──────────────────────────────────────────────────────

fn pump_session_events(
    app: AppHandle,
    inner: Arc<Mutex<Inner>>,
    mut rx: mpsc::UnboundedReceiver<SessionEvent>,
) {
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                SessionEvent::HostReady { room } => {
                    emit_status(
                        &app,
                        format!("Hosting as '{room}' — viewers can connect with this name."),
                    );
                }
                SessionEvent::Viewers(count) => {
                    inner.lock().await.viewers = count;
                    let _ = app.emit("viewers", count);
                }
                SessionEvent::Connected { room } => {
                    emit_status(&app, format!("Connected to '{room}' — feeling what they feel."));
                }
                SessionEvent::Reconnecting { detail } => emit_status(&app, detail),
                SessionEvent::Intensity(_) => {}
                SessionEvent::Log(line) => emit_log(&app, line),
                SessionEvent::Ended { reason } => {
                    emit_status(&app, reason);
                    let _ = app.emit("session-ended", ());
                }
            }
        }
    });
}

// ─── Commands ────────────────────────────────────────────────────────────────

#[tauri::command]
fn list_mods(app: AppHandle) -> Vec<ModInfo> {
    collect_mods(&app)
}

#[tauri::command]
async fn get_snapshot(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    emit_snapshot(&app, &state).await;
    Ok(())
}

#[tauri::command]
async fn start_solo(
    app: AppHandle,
    state: State<'_, AppState>,
    mod_id: String,
) -> Result<(), String> {
    stop_internal(&app, &state).await;
    start_mod_for(&app, &state, &mod_id).await?;
    state.inner.lock().await.mode = Mode::Solo { mod_id };
    emit_snapshot(&app, &state).await;
    Ok(())
}

#[tauri::command]
async fn start_host(
    app: AppHandle,
    state: State<'_, AppState>,
    username: String,
    password: String,
    mod_id: String,
) -> Result<(), String> {
    let name = session::validate_username(&username)?;
    stop_internal(&app, &state).await;
    start_mod_for(&app, &state, &mod_id).await?;

    let (tx, rx) = mpsc::unbounded_channel();
    let session = session::host(&name, &password, &state.bus, tx)
        .await
        .map_err(|e| format!("{e:#}"))?;
    pump_session_events(app.clone(), state.inner.clone(), rx);

    let mut inner = state.inner.lock().await;
    inner.session = Some(session);
    inner.mode = Mode::Host {
        room: session::normalize_room(&name),
        mod_id,
    };
    drop(inner);
    emit_snapshot(&app, &state).await;
    Ok(())
}

#[tauri::command]
async fn start_join(
    app: AppHandle,
    state: State<'_, AppState>,
    username: String,
    password: String,
) -> Result<(), String> {
    let name = session::validate_username(&username)?;
    stop_internal(&app, &state).await;

    let (tx, rx) = mpsc::unbounded_channel();
    let session = session::join(&name, &password, &state.bus, tx)
        .await
        .map_err(|e| format!("{e:#}"))?;
    pump_session_events(app.clone(), state.inner.clone(), rx);

    let mut inner = state.inner.lock().await;
    inner.session = Some(session);
    inner.mode = Mode::Viewer {
        room: session::normalize_room(&name),
    };
    drop(inner);
    emit_snapshot(&app, &state).await;
    Ok(())
}

#[tauri::command]
async fn stop(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    stop_internal(&app, &state).await;
    emit_status(&app, "Stopped.");
    emit_snapshot(&app, &state).await;
    Ok(())
}

async fn stop_internal(app: &AppHandle, state: &AppState) {
    let mut inner = state.inner.lock().await;
    if let Some(m) = inner.running_mod.take() {
        m.stop();
    }
    if let Some(s) = inner.session.take() {
        s.stop();
    }
    inner.viewers = 0;
    inner.mode = Mode::Idle;
    state.bus.kill();
    if let Some(devices) = inner.devices.clone() {
        drop(inner);
        devices.stop_all().await;
    }
    let _ = app.emit("viewers", 0usize);
}

#[tauri::command]
async fn test_buzz(state: State<'_, AppState>) -> Result<(), String> {
    let connected = {
        let inner = state.inner.lock().await;
        inner.devices.as_ref().map(|d| d.connected()).unwrap_or(false)
    };
    if !connected {
        return Err("Toy engine isn't ready yet.".into());
    }
    state.bus.pulse(0.5, 0.6);
    Ok(())
}

#[tauri::command]
async fn scan_devices(state: State<'_, AppState>) -> Result<(), String> {
    let manager = {
        let inner = state.inner.lock().await;
        inner.devices.clone()
    };
    match manager {
        Some(m) => m.scan().await.map_err(|e| format!("{e:#}")),
        None => Err("Toy engine isn't ready yet.".into()),
    }
}

#[tauri::command]
fn open_mods_folder(app: AppHandle) -> Result<String, String> {
    // Prefer the per-user mods dir; create it so the folder always exists.
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("mods");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    #[cfg(target_os = "linux")]
    let opener = "xdg-open";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(target_os = "windows")]
    let opener = "explorer";

    std::process::Command::new(opener)
        .arg(&dir)
        .spawn()
        .map_err(|e| format!("Could not open the file manager: {e}"))?;
    Ok(dir.display().to_string())
}

// ─── Entry ───────────────────────────────────────────────────────────────────

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,iroh=warn,buttplug=warn".into()),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            // The bus spawns its smoothing task, so it must be created inside
            // the tokio runtime (setup itself runs on the main thread).
            let bus = tauri::async_runtime::block_on(async { IntensityBus::new() });
            let inner = Arc::new(Mutex::new(Inner::default()));
            let state = AppState {
                bus: bus.clone(),
                inner: inner.clone(),
            };
            app.manage(state);

            // Seed the per-user mods folder with the bundled mods on first run.
            let handle = app.handle().clone();
            if let (Ok(resources), Ok(data)) =
                (handle.path().resource_dir(), handle.path().app_data_dir())
            {
                let user_mods = data.join("mods");
                let bundled = resources.join("mods");
                let _ = std::fs::create_dir_all(&user_mods);
                if let Ok(entries) = std::fs::read_dir(&bundled) {
                    for entry in entries.flatten() {
                        let target = user_mods.join(entry.file_name());
                        if !target.exists() {
                            let _ = std::fs::copy(entry.path(), target);
                        }
                    }
                }
            }

            // Live intensity meter for the UI.
            {
                let app = app.handle().clone();
                let mut rx = bus.subscribe();
                tauri::async_runtime::spawn(async move {
                    while rx.changed().await.is_ok() {
                        let _ = app.emit("intensity", *rx.borrow_and_update());
                    }
                });
            }

            // Bring up toys in the background.
            {
                let app = app.handle().clone();
                tauri::async_runtime::spawn(device_bringup(app, inner, bus));
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_mods,
            get_snapshot,
            start_solo,
            start_host,
            start_join,
            stop,
            test_buzz,
            scan_devices,
            open_mods_folder,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // Last line of defense: never leave a toy buzzing after exit.
                let state: State<'_, AppState> = app.state();
                state.bus.kill();
                let inner = state.inner.clone();
                tauri::async_runtime::block_on(async move {
                    let mut inner = inner.lock().await;
                    if let Some(m) = inner.running_mod.take() {
                        m.stop();
                    }
                    if let Some(s) = inner.session.take() {
                        s.stop();
                    }
                    if let Some(d) = inner.devices.take() {
                        d.shutdown().await;
                    }
                    #[cfg(feature = "engine")]
                    if let Some(e) = inner.engine.take() {
                        e.stop();
                    }
                });
            }
        });
}
