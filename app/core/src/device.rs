//! Toy connectivity via buttplug.io. This module is a *client*: it talks over a
//! local WebSocket either to our own embedded intiface-engine (started by the
//! app when the `engine` feature is on) or to an already-running Intiface
//! Central. Same code path either way, which is how "support both" stays
//! foolproof.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use buttplug::connector::ButtplugRemoteClientConnector;
use buttplug::device::ClientDeviceOutputCommand;
use buttplug::{ButtplugClient, ButtplugClientEvent, ButtplugWebsocketClientTransport};
use buttplug_core::message::OutputType;
use futures_util::StreamExt;
use serde::Serialize;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

/// Default port of Intiface Central.
pub const INTIFACE_CENTRAL_URL: &str = "ws://127.0.0.1:12345";
/// Port our embedded engine listens on (deliberately not 12345 so both can coexist).
pub const EMBEDDED_ENGINE_PORT: u16 = 12395;

const SCAN_TIME: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub index: u32,
    pub name: String,
    pub can_vibrate: bool,
}

/// Events surfaced to the UI layer.
#[derive(Debug, Clone)]
pub enum DeviceEvent {
    Connected { server: String },
    Disconnected,
    DevicesChanged(Vec<DeviceInfo>),
    Log(String),
}

/// Manages the buttplug client connection and drives all vibrators from the
/// intensity bus. Cloneable; all clones share the same client.
#[derive(Clone)]
pub struct DeviceManager {
    client: Arc<ButtplugClient>,
    cancel: CancellationToken,
}

impl DeviceManager {
    /// Connects to a buttplug server at `url` and starts pumping device events
    /// and intensity updates. Fails with a human-readable error if unreachable.
    pub async fn connect(
        url: &str,
        intensity: watch::Receiver<f64>,
        events: mpsc::UnboundedSender<DeviceEvent>,
    ) -> Result<Self> {
        let client = Arc::new(ButtplugClient::new("VibeLoop"));
        let connector = ButtplugRemoteClientConnector::<ButtplugWebsocketClientTransport>::new(
            ButtplugWebsocketClientTransport::new_insecure_connector(url),
        );
        client
            .connect(connector)
            .await
            .with_context(|| format!("could not reach the toy engine at {url}"))?;

        let server = client.server_name().unwrap_or_else(|| "engine".into());
        let _ = events.send(DeviceEvent::Connected {
            server: server.clone(),
        });

        let manager = Self {
            client,
            cancel: CancellationToken::new(),
        };

        // ── Device add/remove pump ──
        let pump = manager.clone();
        let pump_events = events.clone();
        tokio::spawn(async move {
            let mut stream = pump.client.event_stream();
            loop {
                let event = tokio::select! {
                    _ = pump.cancel.cancelled() => break,
                    e = stream.next() => match e { Some(e) => e, None => break },
                };
                match event {
                    ButtplugClientEvent::DeviceAdded(d) => {
                        let _ = pump_events.send(DeviceEvent::Log(format!(
                            "Toy connected: {}",
                            d.name()
                        )));
                        let _ = pump_events.send(DeviceEvent::DevicesChanged(pump.devices()));
                    }
                    ButtplugClientEvent::DeviceRemoved(d) => {
                        let _ = pump_events.send(DeviceEvent::Log(format!(
                            "Toy disconnected: {}",
                            d.name()
                        )));
                        let _ = pump_events.send(DeviceEvent::DevicesChanged(pump.devices()));
                    }
                    ButtplugClientEvent::ServerDisconnect => {
                        let _ = pump_events.send(DeviceEvent::Disconnected);
                        break;
                    }
                    _ => {}
                }
            }
        });

        // ── Intensity → vibration ──
        let driver = manager.clone();
        let mut rx = intensity;
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = driver.cancel.cancelled() => break,
                    changed = rx.changed() => { if changed.is_err() { break; } }
                }
                let level = *rx.borrow_and_update();
                driver.vibrate_all(level).await;
            }
            // Whatever happens, never leave a toy running.
            driver.stop_all().await;
        });

        // Pick up devices that were already connected before we attached.
        let _ = events.send(DeviceEvent::DevicesChanged(manager.devices()));

        Ok(manager)
    }

    /// Snapshot of currently connected devices.
    pub fn devices(&self) -> Vec<DeviceInfo> {
        self.client
            .devices()
            .iter()
            .map(|(index, d)| DeviceInfo {
                index: *index,
                name: d.name().clone(),
                can_vibrate: d.output_available(OutputType::Vibrate),
            })
            .collect()
    }

    /// Asks the engine to scan for new toys for a few seconds.
    pub async fn scan(&self) -> Result<()> {
        self.client
            .start_scanning()
            .await
            .context("scan failed — is Bluetooth turned on?")?;
        let client = self.client.clone();
        tokio::spawn(async move {
            tokio::time::sleep(SCAN_TIME).await;
            let _ = client.stop_scanning().await;
        });
        Ok(())
    }

    /// Sets vibration on every vibration-capable device; ignores per-device
    /// failures so one flaky toy can't kill the rest.
    pub async fn vibrate_all(&self, level: f64) {
        let level = level.clamp(0.0, 1.0);
        for (_, device) in self.client.devices() {
            if device.output_available(OutputType::Vibrate) {
                let _ = device
                    .run_output(&ClientDeviceOutputCommand::Vibrate(level.into()))
                    .await;
            }
        }
    }

    /// Emergency/final stop for everything.
    pub async fn stop_all(&self) {
        let _ = self.client.stop_all_devices().await;
    }

    pub fn connected(&self) -> bool {
        self.client.connected()
    }

    /// Stops driving devices and disconnects cleanly (toys stopped first).
    pub async fn shutdown(&self) {
        self.cancel.cancel();
        self.stop_all().await;
        let _ = self.client.disconnect().await;
    }
}
