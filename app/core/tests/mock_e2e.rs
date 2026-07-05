//! Whole-product tests with mock data — no hardware, no GUI:
//!
//! 1. `solo_pipeline_from_game_to_toy`: fake tosu → real osu_rewarding.lua →
//!    intensity bus → real buttplug client → mock toy server. Asserts the toy
//!    receives the miss pulse and is released back to zero.
//! 2. `session_pipeline_host_to_viewer_toy` (needs internet): host bus → real
//!    P2P session → viewer bus → buttplug client → viewer's mock toy.
//!
//! The mock toy is a WebSocket server speaking the actual Buttplug v4 protocol
//! using buttplug_core's own message types, so the wire format is exact.

use std::collections::BTreeMap;
use std::time::Duration;

use buttplug_core::message::{
    ButtplugClientMessageV4, ButtplugMessage, ButtplugMessageSpecVersion, ButtplugServerMessageV4,
    DeviceFeature, DeviceFeatureOutput, DeviceFeatureOutputValueProperties, DeviceListV4,
    DeviceMessageInfoV4, OkV0, OutputType, ServerInfoV4,
};
use buttplug_core::util::small_vec_enum_map::SmallVecEnumMap;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use vibeloop_core::device::DeviceManager;
use vibeloop_core::{modengine, session, IntensityBus};

// ─── Mock toy server ─────────────────────────────────────────────────────────

/// Starts a mock Buttplug server with one device ("Mock Lush 3", vibrator with
/// 100 steps). Returns its URL and a stream of received vibrate levels (0–1).
async fn mock_toy() -> (String, mpsc::UnboundedReceiver<f64>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await else {
            return;
        };
        while let Some(Ok(frame)) = ws.next().await {
            if !frame.is_text() {
                continue;
            }
            let text = frame.into_text().unwrap();
            let Ok(messages) = serde_json::from_str::<Vec<ButtplugClientMessageV4>>(&text)
            else {
                panic!("mock toy got unparseable frame: {text}");
            };
            let mut replies: Vec<ButtplugServerMessageV4> = Vec::new();
            for msg in messages {
                match msg {
                    ButtplugClientMessageV4::RequestServerInfo(m) => {
                        let mut info = ServerInfoV4::new(
                            "MockToy Server",
                            ButtplugMessageSpecVersion::Version4,
                            0,
                            0, // max ping time 0 = no ping requirement
                        );
                        info.set_id(m.id());
                        replies.push(ButtplugServerMessageV4::ServerInfo(info));
                    }
                    ButtplugClientMessageV4::RequestDeviceList(m) => {
                        let mut outputs: SmallVecEnumMap<DeviceFeatureOutput, 1> =
                            Default::default();
                        outputs.push(DeviceFeatureOutput::Vibrate(
                            DeviceFeatureOutputValueProperties::new(
                                buttplug_core::util::range::RangeInclusive::new(0, 100),
                            ),
                        ));
                        let feature =
                            DeviceFeature::new(0, "Vibrator", &outputs, &Default::default());
                        let mut features = BTreeMap::new();
                        features.insert(0u32, feature);
                        let device =
                            DeviceMessageInfoV4::new(0, "Mock Lush 3", &None, 10, &features);
                        let mut list = DeviceListV4::new(vec![device]);
                        list.set_id(m.id());
                        replies.push(ButtplugServerMessageV4::DeviceList(list));
                    }
                    ButtplugClientMessageV4::OutputCmd(m) => {
                        if m.command().as_output_type() == OutputType::Vibrate {
                            let _ = tx.send(m.command().value() as f64 / 100.0);
                        }
                        replies.push(ButtplugServerMessageV4::Ok(OkV0::new(m.id())));
                    }
                    other => {
                        replies.push(ButtplugServerMessageV4::Ok(OkV0::new(other.id())));
                    }
                }
            }
            if !replies.is_empty() {
                let json = serde_json::to_string(&replies).unwrap();
                if ws.send(json.into()).await.is_err() {
                    break;
                }
            }
        }
    });

    (format!("ws://127.0.0.1:{port}"), rx)
}

/// Waits until the toy has received a level ≥ `at_least`, returning the peak.
async fn wait_for_level(
    rx: &mut mpsc::UnboundedReceiver<f64>,
    at_least: f64,
    timeout: Duration,
) -> f64 {
    let mut peak: f64 = 0.0;
    let _ = tokio::time::timeout(timeout, async {
        while let Some(v) = rx.recv().await {
            peak = peak.max(v);
            if peak >= at_least {
                break;
            }
        }
    })
    .await;
    peak
}

/// Waits until the toy has been released back to zero.
async fn wait_for_zero(rx: &mut mpsc::UnboundedReceiver<f64>, timeout: Duration) -> bool {
    tokio::time::timeout(timeout, async {
        while let Some(v) = rx.recv().await {
            if v == 0.0 {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false)
}

// ─── Test 1: solo — game to toy ──────────────────────────────────────────────

fn v1_frame(state: u32, h300: u32, geki: u32, miss: u32) -> String {
    serde_json::json!({
        "menu": { "state": state },
        "gameplay": {
            "accuracy": 96.0,
            "hits": { "300": h300, "100": 0, "50": 0, "0": miss, "geki": geki, "katu": 0 }
        }
    })
    .to_string()
}

#[tokio::test]
async fn solo_pipeline_from_game_to_toy() {
    // Fake tosu on the real port the mod dials.
    let tosu = TcpListener::bind(("127.0.0.1", 24050)).await.unwrap();
    tokio::spawn(async move {
        let Ok((stream, _)) = tosu.accept().await else {
            return;
        };
        let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await else {
            return;
        };
        let _ = ws.send(v1_frame(2, 0, 0, 0).into()).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        let _ = ws.send(v1_frame(2, 0, 0, 1).into()).await; // miss → 0.75 pulse
        tokio::time::sleep(Duration::from_secs(10)).await;
    });

    let (toy_url, mut toy_rx) = mock_toy().await;
    let bus = IntensityBus::new();

    let (dev_tx, mut dev_rx) = mpsc::unbounded_channel();
    let manager = DeviceManager::connect(&toy_url, bus.subscribe(), dev_tx)
        .await
        .expect("client must connect to mock toy server");

    // The mock device must become visible and vibration-capable (the client
    // fills its device list asynchronously after the handshake).
    let devices = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let devices = manager.devices();
            if !devices.is_empty() {
                return devices;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("mock device never appeared in the client");
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].name, "Mock Lush 3");
    assert!(devices[0].can_vibrate);
    tokio::spawn(async move { while dev_rx.recv().await.is_some() {} });

    let mods_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("mods");
    let (mod_tx, mut mod_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move { while mod_rx.recv().await.is_some() {} });
    let running = modengine::run_mod(&mods_dir.join("osu_rewarding.lua"), &bus, mod_tx)
        .await
        .expect("mod must load");

    // The miss must reach the (mock) hardware at 0.75, then release to 0.
    let peak = wait_for_level(&mut toy_rx, 0.74, Duration::from_secs(10)).await;
    assert!(
        peak >= 0.74,
        "toy never received the miss pulse (peak {peak})"
    );
    assert!(
        wait_for_zero(&mut toy_rx, Duration::from_secs(10)).await,
        "toy was never released back to zero"
    );

    running.stop();
    manager.shutdown().await;
    bus.shutdown();
}

// ─── Test 2: session — host to viewer's toy over real P2P ───────────────────

#[tokio::test]
#[ignore = "needs internet access"]
async fn session_pipeline_host_to_viewer_toy() {
    let username = format!("vltest-{:08x}", rand::random::<u32>());

    // Host side: just a bus (the mod layer is covered by the solo test).
    let host_bus = IntensityBus::new();
    let (host_tx, mut host_events) = mpsc::unbounded_channel();
    let host = session::host(&username, "mock-pw", &host_bus, host_tx)
        .await
        .expect("host starts");
    tokio::spawn(async move { while host_events.recv().await.is_some() {} });

    // Viewer side: bus + real buttplug client + mock toy.
    let (toy_url, mut toy_rx) = mock_toy().await;
    let viewer_bus = IntensityBus::new();
    let (dev_tx, mut dev_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move { while dev_rx.recv().await.is_some() {} });
    let manager = DeviceManager::connect(&toy_url, viewer_bus.subscribe(), dev_tx)
        .await
        .expect("viewer client must connect to its mock toy");

    let (viewer_tx, mut viewer_events) = mpsc::unbounded_channel();
    let viewer = session::join(&username, "mock-pw", &viewer_bus, viewer_tx)
        .await
        .expect("viewer starts");
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            if let session::SessionEvent::Connected { .. } =
                viewer_events.recv().await.expect("viewer events closed")
            {
                break;
            }
        }
    })
    .await
    .expect("viewer never connected to host");

    // Host's game fires a big moment → the viewer's toy must feel it…
    host_bus.pulse(0.9, 0.8);
    let peak = wait_for_level(&mut toy_rx, 0.85, Duration::from_secs(15)).await;
    assert!(
        peak >= 0.85,
        "viewer toy never got the host's pulse (peak {peak})"
    );

    // …and be released once the pulse fades.
    assert!(
        wait_for_zero(&mut toy_rx, Duration::from_secs(15)).await,
        "viewer toy was never released back to zero"
    );

    viewer.stop();
    host.stop();
    manager.shutdown().await;
    viewer_bus.shutdown();
    host_bus.shutdown();
}
