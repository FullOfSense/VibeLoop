//! Dev tool: run one mod against the REAL game for a few seconds and print
//! everything it does. Usage:
//!     cargo run -p vibeloop-core --example live_mod -- ../mods/war_thunder.lua [seconds]

use std::time::Duration;

#[tokio::main]
async fn main() {
    let path = std::env::args().nth(1).expect("usage: live_mod <mod.lua> [seconds]");
    let secs: u64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(12);

    let bus = vibeloop_core::IntensityBus::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let running = vibeloop_core::modengine::run_mod(std::path::Path::new(&path), &bus, tx)
        .await
        .expect("mod failed to load");

    let mut watch = bus.subscribe();
    tokio::spawn(async move {
        let mut last = 0.0f64;
        while watch.changed().await.is_ok() {
            let v = *watch.borrow_and_update();
            if (v - last).abs() > 0.02 {
                println!("[level] {v:.2}");
                last = v;
            }
        }
    });
    tokio::spawn(async move {
        while let Some(e) = rx.recv().await {
            println!("[event] {e:?}");
        }
    });

    tokio::time::sleep(Duration::from_secs(secs)).await;
    running.stop();
    bus.shutdown();
}
