//! The intensity bus: single source of truth for "how strong should the buzz be
//! right now". Mods (or a remote session) write targets into it; the bus applies
//! the smoothing curve and fans the result out to every consumer (local devices,
//! session broadcast, UI meter) through a `watch` channel.
//!
//! Smoothing matches the original Python engine: intensity jumps up instantly,
//! decays down exponentially, and snaps to zero below a small threshold so toys
//! don't hum forever at 2%.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

const TICK: Duration = Duration::from_millis(50);
const SMOOTH_DOWN: f64 = 0.35;
const SNAP_TO_ZERO: f64 = 0.04;

#[derive(Debug)]
struct BusState {
    /// Sustained intensity level (e.g. win pattern, spinner) — set/cleared by mods.
    base: f64,
    /// One-shot pulse; a new pulse replaces the previous one.
    pulse_intensity: f64,
    pulse_until: Instant,
    /// Hard ceiling applied after mixing (user safety cap).
    max: f64,
    current: f64,
}

/// Cloneable handle used by producers (mods, session client) and the UI.
#[derive(Clone)]
pub struct IntensityBus {
    state: Arc<Mutex<BusState>>,
    rx: watch::Receiver<f64>,
    cancel: CancellationToken,
}

impl IntensityBus {
    /// Creates the bus and starts its smoothing task on the current runtime.
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(BusState {
            base: 0.0,
            pulse_intensity: 0.0,
            pulse_until: Instant::now(),
            max: 1.0,
            current: 0.0,
        }));
        let (tx, rx) = watch::channel(0.0f64);
        let cancel = CancellationToken::new();

        let task_state = state.clone();
        let task_cancel = cancel.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(TICK);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut last_sent = -1.0f64;
            loop {
                tokio::select! {
                    _ = interval.tick() => {}
                    _ = task_cancel.cancelled() => {
                        let _ = tx.send(0.0);
                        return;
                    }
                }
                let value = {
                    let mut s = task_state.lock().expect("bus lock poisoned");
                    let pulse = if Instant::now() < s.pulse_until {
                        s.pulse_intensity
                    } else {
                        0.0
                    };
                    let target = s.base.max(pulse).clamp(0.0, 1.0).min(s.max);
                    if target >= s.current {
                        s.current = target;
                    } else {
                        s.current += (target - s.current) * SMOOTH_DOWN;
                        if s.current < SNAP_TO_ZERO {
                            s.current = 0.0;
                        }
                    }
                    (s.current * 100.0).round() / 100.0
                };
                if (value - last_sent).abs() > 0.005 {
                    last_sent = value;
                    let _ = tx.send(value);
                }
            }
        });

        Self { state, rx, cancel }
    }

    /// Sets the sustained intensity level (0.0–1.0). Stays until changed.
    /// Non-finite values (NaN/inf, e.g. from a buggy mod) are ignored — NaN
    /// survives `clamp` and would poison the smoothing math and the devices.
    pub fn set_base(&self, level: f64) {
        if !level.is_finite() {
            return;
        }
        let mut s = self.state.lock().expect("bus lock poisoned");
        s.base = level.clamp(0.0, 1.0);
    }

    /// Fires a one-shot pulse; replaces any pulse still running.
    pub fn pulse(&self, level: f64, seconds: f64) {
        if !level.is_finite() || !seconds.is_finite() {
            return;
        }
        let mut s = self.state.lock().expect("bus lock poisoned");
        s.pulse_intensity = level.clamp(0.0, 1.0);
        s.pulse_until = Instant::now() + Duration::from_secs_f64(seconds.clamp(0.0, 30.0));
    }

    /// Zeroes everything immediately (used on stop / host disconnect).
    pub fn kill(&self) {
        let mut s = self.state.lock().expect("bus lock poisoned");
        s.base = 0.0;
        s.pulse_intensity = 0.0;
        s.pulse_until = Instant::now();
        s.current = 0.0;
    }

    /// User safety ceiling, applied after all mixing (1.0 = no cap).
    pub fn set_max(&self, max: f64) {
        if !max.is_finite() {
            return;
        }
        let mut s = self.state.lock().expect("bus lock poisoned");
        s.max = max.clamp(0.0, 1.0);
    }

    /// Subscribe to smoothed intensity updates (deduplicated, ~20 Hz max).
    pub fn subscribe(&self) -> watch::Receiver<f64> {
        self.rx.clone()
    }

    /// Stops the smoothing task and sends a final 0.
    pub fn shutdown(&self) {
        self.cancel.cancel();
    }
}

impl Default for IntensityBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pulse_rises_and_decays() {
        let bus = IntensityBus::new();
        let mut rx = bus.subscribe();
        bus.pulse(0.8, 0.2);
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                rx.changed().await.unwrap();
                if (*rx.borrow() - 0.8).abs() < 0.01 {
                    break;
                }
            }
        })
        .await
        .expect("never reached pulse level");
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                rx.changed().await.unwrap();
                if *rx.borrow() == 0.0 {
                    break;
                }
            }
        })
        .await
        .expect("never decayed to zero");
        bus.shutdown();
    }

    #[tokio::test]
    async fn non_finite_inputs_are_ignored() {
        let bus = IntensityBus::new();
        // None of these may panic (Duration::from_secs_f64(NaN) would) or
        // poison the bus with NaN.
        bus.set_base(f64::NAN);
        bus.pulse(f64::INFINITY, f64::NAN);
        bus.pulse(0.5, f64::INFINITY);
        bus.set_max(f64::NAN);
        tokio::time::sleep(Duration::from_millis(120)).await;
        let v = *bus.subscribe().borrow();
        assert!(v.is_finite(), "bus emitted non-finite value {v}");
        bus.shutdown();
    }

    #[tokio::test]
    async fn max_caps_base() {
        let bus = IntensityBus::new();
        let mut rx = bus.subscribe();
        bus.set_max(0.5);
        bus.set_base(1.0);
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                rx.changed().await.unwrap();
                let v = *rx.borrow();
                assert!(v <= 0.5 + 1e-9, "cap exceeded: {v}");
                if (v - 0.5).abs() < 0.01 {
                    break;
                }
            }
        })
        .await
        .expect("never reached capped level");
        bus.shutdown();
    }
}
