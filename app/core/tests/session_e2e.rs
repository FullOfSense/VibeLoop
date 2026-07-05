//! Full P2P session test over the real iroh network: host a room, join it by
//! username+password only, and verify intensity flows host → viewer.
//!
//! Needs internet (uses n0's public discovery), so it's `#[ignore]` by default:
//! run with `cargo test -p vibeloop-core --test session_e2e -- --ignored`

use std::time::Duration;

use tokio::sync::mpsc;
use vibeloop_core::{session, IntensityBus};

#[tokio::test]
#[ignore = "needs internet access"]
async fn host_and_join_by_username() {
    // Random room name so parallel CI runs can't collide.
    let username = format!("vltest-{:08x}", rand::random::<u32>());
    let password = "e2e-secret";

    let host_bus = IntensityBus::new();
    let (host_tx, mut host_events) = mpsc::unbounded_channel();
    let host = session::host(&username, password, &host_bus, host_tx)
        .await
        .expect("host should start");

    let viewer_bus = IntensityBus::new();
    let (viewer_tx, mut viewer_events) = mpsc::unbounded_channel();
    let viewer = session::join(&username, password, &viewer_bus, viewer_tx)
        .await
        .expect("viewer should start");

    // Wait for the viewer to connect (discovery can take a few seconds).
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            match viewer_events.recv().await.expect("viewer events closed") {
                session::SessionEvent::Connected { .. } => break,
                other => println!("viewer: {other:?}"),
            }
        }
    })
    .await
    .expect("viewer never connected — discovery or hole punching failed");

    // Host raises intensity; viewer's bus must follow.
    host_bus.set_base(0.66);
    let mut rx = viewer_bus.subscribe();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            rx.changed().await.unwrap();
            if (*rx.borrow() - 0.66).abs() < 0.02 {
                break;
            }
        }
    })
    .await
    .expect("intensity never arrived at the viewer");

    // Host sees exactly one viewer.
    let saw_viewer = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let session::SessionEvent::Viewers(1) =
                host_events.recv().await.expect("host events closed")
            {
                break true;
            }
        }
    })
    .await
    .unwrap_or(false);
    assert!(saw_viewer, "host never reported a viewer");

    viewer.stop();
    host.stop();
    host_bus.shutdown();
    viewer_bus.shutdown();
}
