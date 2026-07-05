//! Game mod engine. A mod is a single Lua file dropped into the `mods/` folder:
//!
//! ```lua
//! -- @name: osu! — Rewarding
//! -- @game: osu!
//! -- @description: Buzz on hits, stronger for better judgements.
//!
//! sources = {
//!   { id = "v1", url = "ws://127.0.0.1:24050/ws" },
//! }
//!
//! function on_message(source, data)  -- data = decoded JSON as a Lua table
//!   vibe.pulse(0.5, 0.14)
//! end
//!
//! function on_tick(now)              -- optional, ~20 Hz, now = seconds
//!   vibe.set(0.2)
//! end
//! ```
//!
//! The `vibe` API: `set(level)`, `pulse(level, seconds)`, `log(msg)`,
//! `status(msg)`, `now()`. The engine handles WebSocket connect/reconnect for
//! every declared source and reports source state to the UI, so mods contain
//! zero networking or error-handling boilerplate.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures_util::StreamExt;
use mlua::{Function, Lua, LuaOptions, LuaSerdeExt, StdLib, Table};
use serde::Serialize;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::bus::IntensityBus;

const TICK: Duration = Duration::from_millis(50);
const RECONNECT_DELAY: Duration = Duration::from_secs(3);

/// Metadata parsed from `-- @key: value` header comments, no code execution.
#[derive(Debug, Clone, Serialize)]
pub struct ModInfo {
    pub id: String,
    pub name: String,
    pub game: String,
    pub description: String,
    /// One-line "what you need to run" hint (`-- @setup:` header), e.g.
    /// "Run tosu alongside osu!". Shown in the UI when the mod is selected.
    pub setup: String,
    pub path: PathBuf,
}

/// Events surfaced to the UI layer.
#[derive(Debug, Clone)]
pub enum ModEvent {
    /// A declared source connected (e.g. tosu found).
    SourceUp { source: String },
    /// A declared source is unreachable; the engine keeps retrying.
    SourceDown { source: String, detail: String },
    /// Mod called vibe.status() — one-line state for the UI.
    Status(String),
    /// Mod called vibe.log() or the engine logged something.
    Log(String),
    /// The mod crashed (Lua error). The engine stops the mod; UI should show it.
    Failed(String),
}

/// Scans a directory for `*.lua` mods and reads their headers.
pub fn scan_mods(dir: &Path) -> Vec<ModInfo> {
    let mut mods = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return mods;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("lua") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let header = |key: &str| -> Option<String> {
            text.lines()
                .take(20)
                .filter_map(|l| l.trim().strip_prefix("--"))
                .filter_map(|l| l.trim().strip_prefix(&format!("@{key}:")))
                .map(|v| v.trim().to_string())
                .next()
        };
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("mod")
            .to_string();
        mods.push(ModInfo {
            id: stem.clone(),
            name: header("name").unwrap_or(stem),
            game: header("game").unwrap_or_else(|| "Unknown game".into()),
            description: header("description").unwrap_or_default(),
            setup: header("setup").unwrap_or_default(),
            path,
        });
    }
    mods.sort_by(|a, b| a.name.cmp(&b.name));
    mods
}

/// A running mod. Dropping/stopping tears down its sources and zeroes the bus.
pub struct RunningMod {
    cancel: CancellationToken,
}

impl RunningMod {
    pub fn stop(&self) {
        self.cancel.cancel();
    }
}

impl Drop for RunningMod {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// Loads and runs a mod file. Returns after the Lua chunk has been validated;
/// sources and the tick loop run in background tasks until stopped.
pub async fn run_mod(
    path: &Path,
    bus: &IntensityBus,
    events: mpsc::UnboundedSender<ModEvent>,
) -> Result<RunningMod> {
    let source_code = std::fs::read_to_string(path)
        .with_context(|| format!("could not read mod file {}", path.display()))?;
    let mod_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("mod")
        .to_string();

    // Sandbox: mods get math/string/table/utf8 + the base library only — no
    // io, no os, no require/package, no debug. A downloaded mod can drive the
    // vibe API, not the filesystem or shell.
    let lua = Lua::new_with(
        StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8,
        LuaOptions::default(),
    )
    .context("could not create Lua sandbox")?;
    let started = Instant::now();

    // ── vibe API ──
    {
        let vibe = lua.create_table()?;
        let b = bus.clone();
        vibe.set(
            "set",
            lua.create_function(move |_, level: f64| {
                b.set_base(level);
                Ok(())
            })?,
        )?;
        let b = bus.clone();
        vibe.set(
            "pulse",
            lua.create_function(move |_, (level, seconds): (f64, f64)| {
                b.pulse(level, seconds);
                Ok(())
            })?,
        )?;
        let ev = events.clone();
        vibe.set(
            "log",
            lua.create_function(move |_, msg: String| {
                let _ = ev.send(ModEvent::Log(msg));
                Ok(())
            })?,
        )?;
        let ev = events.clone();
        vibe.set(
            "status",
            lua.create_function(move |_, msg: String| {
                let _ = ev.send(ModEvent::Status(msg));
                Ok(())
            })?,
        )?;
        vibe.set(
            "now",
            lua.create_function(move |_, ()| Ok(started.elapsed().as_secs_f64()))?,
        )?;
        lua.globals().set("vibe", vibe)?;
    }

    lua.load(&source_code)
        .set_name(&mod_name)
        .exec()
        .with_context(|| format!("mod '{mod_name}' has an error"))?;

    // ── Declared sources ──
    let mut sources: Vec<(String, String)> = Vec::new();
    if let Ok(table) = lua.globals().get::<Table>("sources") {
        for pair in table.sequence_values::<Table>() {
            let entry = pair?;
            let id: String = entry.get("id").context("source needs an `id`")?;
            let url: String = entry.get("url").context("source needs a `url`")?;
            sources.push((id, url));
        }
    }
    let has_tick = lua.globals().get::<Function>("on_tick").is_ok();
    if sources.is_empty() && !has_tick {
        anyhow::bail!(
            "mod '{mod_name}' declares no sources and no on_tick — nothing would ever happen"
        );
    }

    let cancel = CancellationToken::new();
    let (msg_tx, mut msg_rx) = mpsc::channel::<(String, String)>(256);

    for (id, url) in &sources {
        tokio::spawn(source_loop(
            id.clone(),
            url.clone(),
            msg_tx.clone(),
            events.clone(),
            cancel.clone(),
        ));
    }

    // ── Lua driver: owns the interpreter, feeds messages + ticks ──
    let bus = bus.clone();
    let driver_cancel = cancel.clone();
    tokio::spawn(async move {
        let on_message: Option<Function> = lua.globals().get("on_message").ok();
        let on_tick: Option<Function> = lua.globals().get("on_tick").ok();
        let mut interval = tokio::time::interval(TICK);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // With no sources (tick-only mods) every sender is already dropped;
        // recv() would yield None forever, so gate the branch off instead.
        let mut msgs_open = true;

        loop {
            let step: Result<(), mlua::Error> = tokio::select! {
                _ = driver_cancel.cancelled() => break,
                msg = msg_rx.recv(), if msgs_open => {
                    let Some((source_id, text)) = msg else {
                        msgs_open = false;
                        continue;
                    };
                    match (&on_message, serde_json::from_str::<serde_json::Value>(&text)) {
                        (Some(f), Ok(json)) => {
                            lua.to_value(&json)
                                .and_then(|value| f.call::<()>((source_id, value)))
                        }
                        _ => Ok(()),
                    }
                }
                _ = interval.tick() => {
                    match &on_tick {
                        Some(f) => f.call::<()>(started.elapsed().as_secs_f64()),
                        None => Ok(()),
                    }
                }
            };
            if let Err(e) = step {
                let _ = events.send(ModEvent::Failed(format!(
                    "Mod '{mod_name}' crashed: {e}"
                )));
                break;
            }
        }
        bus.kill();
    });

    Ok(RunningMod { cancel })
}

/// Connects to one WebSocket source, forwards text frames, reconnects forever.
/// When several frames are queued we keep only the freshest (game state
/// snapshots supersede each other; stale ones just add latency).
async fn source_loop(
    id: String,
    url: String,
    msg_tx: mpsc::Sender<(String, String)>,
    events: mpsc::UnboundedSender<ModEvent>,
    cancel: CancellationToken,
) {
    let mut announced_down = false;
    loop {
        if cancel.is_cancelled() {
            return;
        }
        let connect = tokio::select! {
            _ = cancel.cancelled() => return,
            c = tokio_tungstenite::connect_async(&url) => c,
        };
        match connect {
            Ok((mut ws, _)) => {
                let _ = events.send(ModEvent::SourceUp { source: id.clone() });
                loop {
                    let frame = tokio::select! {
                        _ = cancel.cancelled() => return,
                        f = ws.next() => f,
                    };
                    match frame {
                        Some(Ok(msg)) if msg.is_text() => {
                            let text = msg.into_text().map(|t| t.to_string()).unwrap_or_default();
                            // Drop the queued frame if the mod is behind; the
                            // next snapshot carries the full state anyway.
                            let _ = msg_tx.try_send((id.clone(), text));
                        }
                        Some(Ok(_)) => {}
                        Some(Err(_)) | None => break,
                    }
                }
                let _ = events.send(ModEvent::SourceDown {
                    source: id.clone(),
                    detail: "connection lost — retrying".into(),
                });
                announced_down = true;
            }
            Err(e) => {
                if !announced_down {
                    let _ = events.send(ModEvent::SourceDown {
                        source: id.clone(),
                        detail: format!("can't reach {url} ({e}) — retrying"),
                    });
                    announced_down = true;
                }
            }
        }
        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep(RECONNECT_DELAY) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_headers_without_executing() {
        let dir = std::env::temp_dir().join(format!("vibeloop-scan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("test_mod.lua"),
            "-- @name: Test Mod\n-- @game: TestGame\n-- @description: hi\nerror('never runs at scan')\n",
        )
        .unwrap();
        let mods = scan_mods(&dir);
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "Test Mod");
        assert_eq!(mods[0].game, "TestGame");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn mod_without_sources_is_rejected() {
        let dir = std::env::temp_dir().join(format!("vibeloop-nosrc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty.lua");
        std::fs::write(&path, "-- @name: Empty\n").unwrap();
        let bus = IntensityBus::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let result = run_mod(&path, &bus, tx).await;
        assert!(result.is_err());
        bus.shutdown();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn sandbox_blocks_io_os_and_require() {
        let dir = std::env::temp_dir().join(format!("vibeloop-sandbox-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (name, code) in [
            ("os.lua", "os.execute('true')"),
            ("io.lua", "io.open('/etc/passwd')"),
            ("require.lua", "require('socket')"),
        ] {
            let path = dir.join(name);
            std::fs::write(&path, code).unwrap();
            let bus = IntensityBus::new();
            let (tx, _rx) = mpsc::unbounded_channel();
            let result = run_mod(&path, &bus, tx).await;
            assert!(result.is_err(), "{name} must be blocked by the sandbox");
            bus.shutdown();
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn lua_error_is_reported_not_panicking() {
        let dir = std::env::temp_dir().join(format!("vibeloop-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.lua");
        std::fs::write(&path, "this is not lua at all (").unwrap();
        let bus = IntensityBus::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let result = run_mod(&path, &bus, tx).await;
        assert!(result.is_err(), "syntax error must fail loudly");
        bus.shutdown();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
