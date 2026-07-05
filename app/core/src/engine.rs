//! Embedded intiface-engine: the exact same engine Intiface Central runs,
//! compiled into the app. Started on a private local port when no external
//! Intiface Central is detected, so the user never has to install anything.

use std::sync::Arc;

use anyhow::Result;
use intiface_engine::{EngineOptionsBuilder, IntifaceEngine};
use tokio::sync::mpsc;

use crate::device::EMBEDDED_ENGINE_PORT;

/// Handle to the running embedded engine.
pub struct EmbeddedEngine {
    engine: Arc<IntifaceEngine>,
}

impl EmbeddedEngine {
    /// Starts the engine on [`EMBEDDED_ENGINE_PORT`] with every hardware path
    /// enabled that Intiface Central would offer (Bluetooth LE, Lovense USB
    /// dongle in both serial and HID mode, Lovense Connect, serial, HID).
    ///
    /// Resolves once the engine task is running; any fatal engine error is
    /// reported through `error_tx` so the UI can react (e.g. "Bluetooth off").
    pub async fn start(error_tx: mpsc::UnboundedSender<String>) -> Result<Self> {
        let options = EngineOptionsBuilder::default()
            .websocket_port(EMBEDDED_ENGINE_PORT)
            .server_name("VibeLoop Embedded Engine")
            .use_bluetooth_le(true)
            .use_lovense_dongle_serial(true)
            .use_lovense_dongle_hid(true)
            .use_lovense_connect(true)
            .use_serial_port(true)
            .use_hid(true)
            .finish();

        let engine = Arc::new(IntifaceEngine::default());
        let run_engine = engine.clone();
        tokio::spawn(async move {
            if let Err(e) = run_engine.run(&options, None, &None).await {
                let _ = error_tx.send(format!("Toy engine stopped: {e:?}"));
            }
        });

        Ok(Self { engine })
    }

    pub fn stop(&self) {
        self.engine.stop();
    }
}

impl Drop for EmbeddedEngine {
    fn drop(&mut self) {
        self.engine.stop();
    }
}
