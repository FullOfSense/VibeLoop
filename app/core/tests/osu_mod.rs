//! End-to-end mod test: a fake tosu server feeds osu! frames to the real
//! osu_rewarding.lua mod, and we assert the intensity bus reacts.

use std::path::PathBuf;
use std::time::Duration;

use futures_util::SinkExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use vibeloop_core::{modengine, IntensityBus};

fn mods_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("mods")
}

/// Serves one websocket connection on the given port and sends `frames`.
async fn fake_tosu(port: u16, frames: Vec<String>) {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("tosu test port busy");
    tokio::spawn(async move {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await else {
            return;
        };
        for frame in frames {
            let _ = ws.send(frame.into()).await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        // Keep the socket open so the mod doesn't log reconnects mid-test.
        tokio::time::sleep(Duration::from_secs(10)).await;
    });
}

fn v1_frame(state: u32, h300: u32, geki: u32, miss: u32) -> String {
    serde_json::json!({
        "menu": { "state": state },
        "gameplay": {
            "accuracy": 98.5,
            "hits": { "300": h300, "100": 0, "50": 0, "0": miss, "geki": geki, "katu": 0 }
        }
    })
    .to_string()
}

#[tokio::test]
async fn osu_punishing_loads_cleanly() {
    let mod_path = mods_dir().join("osu_punishing.lua");
    let bus = IntensityBus::new();
    let (tx, mut events) = mpsc::unbounded_channel();
    tokio::spawn(async move { while events.recv().await.is_some() {} });
    let running = modengine::run_mod(&mod_path, &bus, tx)
        .await
        .expect("punishing variant should load");
    running.stop();
    bus.shutdown();
}

#[tokio::test]
async fn osu_rewarding_pulses_on_hits() {
    let mod_path = mods_dir().join("osu_rewarding.lua");
    assert!(mod_path.exists(), "expected {}", mod_path.display());

    // Baseline frame (in map, no hits yet), then a perfect hit, then a miss.
    fake_tosu(
        24050,
        vec![
            v1_frame(2, 0, 0, 0),
            v1_frame(2, 1, 1, 0), // geki → perfect pulse (0.50)
            v1_frame(2, 1, 1, 1), // miss → 0.75 pulse
        ],
    )
    .await;

    let bus = IntensityBus::new();
    let mut rx = bus.subscribe();
    let (tx, mut events) = mpsc::unbounded_channel();
    let running = modengine::run_mod(&mod_path, &bus, tx)
        .await
        .expect("mod should load");

    // Drain events in the background so nothing blocks.
    tokio::spawn(async move { while events.recv().await.is_some() {} });

    // The perfect hit then the miss should push intensity to ≥ 0.5 and ≥ 0.75.
    let mut peak: f64 = 0.0;
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            rx.changed().await.unwrap();
            let v = *rx.borrow();
            peak = peak.max(v);
            if peak >= 0.74 {
                break;
            }
        }
    })
    .await;

    running.stop();
    bus.shutdown();
    assert!(
        result.is_ok(),
        "expected a miss pulse ≥ 0.75, highest intensity seen was {peak}"
    );
}
