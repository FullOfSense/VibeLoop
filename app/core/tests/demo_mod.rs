//! The demo test mod needs no game and no sources — on_tick alone must
//! drive the intensity bus. This is the mod users run to verify their setup.

use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::mpsc;
use vibeloop_core::{modengine, IntensityBus};

fn mods_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("mods")
}

#[tokio::test]
async fn demo_mod_buzzes_without_any_game() {
    let mod_path = mods_dir().join("demo_test.lua");
    assert!(mod_path.exists(), "expected {}", mod_path.display());

    let bus = IntensityBus::new();
    let mut rx = bus.subscribe();
    let (tx, mut events) = mpsc::unbounded_channel();
    let running = modengine::run_mod(&mod_path, &bus, tx)
        .await
        .expect("demo mod should load despite having no sources");
    tokio::spawn(async move { while events.recv().await.is_some() {} });

    // Phase one fires 40% pulses immediately; one must reach the bus fast.
    let mut peak: f64 = 0.0;
    let result = tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            rx.changed().await.unwrap();
            let v = *rx.borrow();
            peak = peak.max(v);
            if peak >= 0.35 {
                break;
            }
        }
    })
    .await;

    running.stop();
    bus.shutdown();
    assert!(
        result.is_ok(),
        "expected a demo pulse ≥ 0.40, highest intensity seen was {peak}"
    );
}
