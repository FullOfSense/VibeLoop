//! Tests for the Tier A game mods, without any game installed:
//!
//! - every shipped mod must load under the sandbox (syntax + source specs)
//! - CS2 / VRChat / War Thunder / Beat Saber mods are driven by fake games
//!   speaking the real protocols on the real ports
//! - the League mod's logic runs against fixture JSON directly (its API is
//!   HTTPS-only, so instead of faking TLS we call on_message like the
//!   engine would)

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::SinkExt;
use mlua::LuaSerdeExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use vibeloop_core::modengine::run_mod;
use vibeloop_core::IntensityBus;

fn mods_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mods")
}

async fn expect_level(bus: &IntensityBus, level: f64, secs: u64, what: &str) {
    let mut rx = bus.subscribe();
    tokio::time::timeout(Duration::from_secs(secs), async {
        loop {
            if *rx.borrow() >= level {
                break;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what}: bus never reached {level}"));
}

async fn expect_zero(bus: &IntensityBus, secs: u64, what: &str) {
    let mut rx = bus.subscribe();
    tokio::time::timeout(Duration::from_secs(secs), async {
        loop {
            if *rx.borrow() == 0.0 {
                break;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what}: bus never released to zero"));
}

/// One sequential test: sections share fixed localhost ports, so they must
/// not run in parallel with each other.
#[tokio::test]
async fn tier_a_mods_react_to_fake_games() {
    // ── Every shipped mod loads under the sandbox ──
    for entry in std::fs::read_dir(mods_dir()).unwrap().flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("lua") {
            continue;
        }
        let bus = IntensityBus::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let running = run_mod(&path, &bus, tx)
            .await
            .unwrap_or_else(|e| panic!("{} failed to load: {e:#}", path.display()));
        running.stop();
        bus.shutdown();
        // Let listeners release their fixed ports before the next mod.
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;

    // ── War Thunder: fake telemetry on 8111, 6 G pull → sustained buzz ──
    // Skipped when the REAL game is running on this machine (it owns 8111).
    if let Ok(server) = tokio::net::TcpListener::bind("127.0.0.1:8111").await {
        let server_task = tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = server.accept().await else { return };
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    loop {
                        let Ok(n) = sock.read(&mut buf).await else { return };
                        if n == 0 {
                            return;
                        }
                        let body = r#"{"valid":true,"Ny":6.0}"#;
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                            body.len()
                        );
                        if sock.write_all(resp.as_bytes()).await.is_err() {
                            return;
                        }
                    }
                });
            }
        });
        let bus = IntensityBus::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let running = run_mod(&mods_dir().join("war_thunder.lua"), &bus, tx).await.unwrap();
        expect_level(&bus, 0.25, 5, "war thunder G-load").await;
        running.stop();
        bus.shutdown();
        server_task.abort();
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    // ── CS2: fake GSI POSTs, 53 damage → strong pulse ──
    {
        let bus = IntensityBus::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let running = run_mod(&mods_dir().join("counterstrike2.lua"), &bus, tx).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        let mut sock = tokio::net::TcpStream::connect(("127.0.0.1", 3902)).await.unwrap();
        for health in [100, 47] {
            let body = format!(
                r#"{{"provider":{{"steamid":"765"}},"player":{{"steamid":"765","team":"CT","state":{{"health":{health},"round_kills":0,"flashed":0,"burning":0}}}},"round":{{"phase":"live"}}}}"#
            );
            let req = format!(
                "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(req.as_bytes()).await.unwrap();
            let mut resp = [0u8; 512];
            let _ = sock.read(&mut resp).await.unwrap();
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        expect_level(&bus, 0.6, 5, "cs2 damage pulse").await;
        running.stop();
        bus.shutdown();
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    // ── VRChat: OSC parameter drives intensity, silence releases it ──
    {
        let bus = IntensityBus::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let running = run_mod(&mods_dir().join("vrchat.lua"), &bus, tx).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let packet = rosc::encoder::encode(&rosc::OscPacket::Message(rosc::OscMessage {
            addr: "/avatar/parameters/VibeLoop".into(),
            args: vec![rosc::OscType::Float(0.8)],
        }))
        .unwrap();
        for _ in 0..5 {
            sock.send_to(&packet, ("127.0.0.1", 9001)).await.unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        expect_level(&bus, 0.75, 5, "vrchat contact").await;
        // No more packets: the mod must assume the release was lost and stop.
        expect_zero(&bus, 6, "vrchat lost-release safety").await;
        running.stop();
        bus.shutdown();
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    // ── Beat Saber: fake DataPuller, a miss → sting ──
    {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:2946").await.unwrap();
        let (map_tx, mut map_rx) = mpsc::unbounded_channel::<()>();
        let server_task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { return };
                let map_tx = map_tx.clone();
                tokio::spawn(async move {
                    let mut path = String::new();
                    let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(
                        stream,
                        |req: &tokio_tungstenite::tungstenite::handshake::server::Request,
                         resp| {
                            path = req.uri().path().to_string();
                            Ok(resp)
                        },
                    )
                    .await
                    else {
                        return;
                    };
                    if path.ends_with("MapData") {
                        let _ = ws
                            .send(r#"{"InLevel":true,"LevelPaused":false}"#.into())
                            .await;
                        let _ = map_tx.send(());
                        // Keep the socket open.
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    } else {
                        // LiveData: a clean cut, then a miss.
                        tokio::time::sleep(Duration::from_millis(400)).await;
                        let _ = ws
                            .send(r#"{"Combo":1,"Misses":0,"PlayerHealth":100}"#.into())
                            .await;
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        let _ = ws
                            .send(r#"{"Combo":0,"Misses":1,"PlayerHealth":90}"#.into())
                            .await;
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    }
                });
            }
        });
        let bus = IntensityBus::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let running = run_mod(&mods_dir().join("beat_saber.lua"), &bus, tx).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), map_rx.recv())
            .await
            .expect("mod never connected to MapData");
        expect_level(&bus, 0.6, 5, "beat saber miss").await;
        running.stop();
        bus.shutdown();
        server_task.abort();
    }
}

/// Drives the League mod's Lua logic directly with fixture payloads —
/// exactly what the engine does after polling, minus the TLS transport.
#[tokio::test]
async fn league_logic_reacts_to_fixtures() {
    let source = std::fs::read_to_string(mods_dir().join("league_of_legends.lua")).unwrap();
    let lua = mlua::Lua::new();
    let calls: Arc<Mutex<Vec<(String, f64)>>> = Arc::new(Mutex::new(Vec::new()));

    let vibe = lua.create_table().unwrap();
    let c = calls.clone();
    vibe.set(
        "pulse",
        lua.create_function(move |_, (level, _secs): (f64, f64)| {
            c.lock().unwrap().push(("pulse".into(), level));
            Ok(())
        })
        .unwrap(),
    )
    .unwrap();
    let c = calls.clone();
    vibe.set(
        "set",
        lua.create_function(move |_, level: f64| {
            c.lock().unwrap().push(("set".into(), level));
            Ok(())
        })
        .unwrap(),
    )
    .unwrap();
    vibe.set("log", lua.create_function(|_, _: String| Ok(())).unwrap()).unwrap();
    vibe.set("status", lua.create_function(|_, _: String| Ok(())).unwrap()).unwrap();
    vibe.set("now", lua.create_function(|_, ()| Ok(0.0f64)).unwrap()).unwrap();
    lua.globals().set("vibe", vibe).unwrap();
    lua.load(&source).exec().unwrap();
    let on_message: mlua::Function = lua.globals().get("on_message").unwrap();

    let payload = |hp: f64, kills: i64, deaths: i64, events: &str| {
        serde_json::from_str::<serde_json::Value>(&format!(
            r#"{{
              "activePlayer": {{
                "riotId": "Me#EUW",
                "championStats": {{ "currentHealth": {hp}, "maxHealth": 1000 }}
              }},
              "allPlayers": [
                {{ "riotId": "Me#EUW", "scores": {{ "kills": {kills}, "deaths": {deaths} }} }},
                {{ "riotId": "Foe#EUW", "scores": {{ "kills": 0, "deaths": 0 }} }}
              ],
              "events": {{ "Events": [{events}] }}
            }}"#
        ))
        .unwrap()
    };
    let send = |json: &serde_json::Value| {
        let value = lua.to_value(json).unwrap();
        on_message.call::<()>(("live", value)).unwrap();
    };

    // Baseline, then a 400-damage hit, a kill, a death, and the win.
    send(&payload(1000.0, 0, 0, ""));
    send(&payload(600.0, 0, 0, ""));
    send(&payload(600.0, 1, 0, ""));
    send(&payload(0.0, 1, 1, ""));
    send(&payload(1000.0, 1, 1, r#"{"EventName":"GameEnd","Result":"Win"}"#));

    let calls = calls.lock().unwrap();
    let pulses: Vec<f64> = calls.iter().filter(|(k, _)| k == "pulse").map(|(_, v)| *v).collect();
    assert!(
        pulses.iter().any(|p| (0.7..=0.85).contains(p)),
        "400 damage should pulse hard, got {pulses:?}"
    );
    assert!(
        pulses.iter().any(|p| (*p - 0.6).abs() < 0.01),
        "kill should pulse 0.6, got {pulses:?}"
    );
    assert!(
        pulses.iter().any(|p| *p >= 0.9),
        "death should pulse ≥0.9, got {pulses:?}"
    );
}
