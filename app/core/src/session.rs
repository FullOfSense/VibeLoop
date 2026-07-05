//! Shared sessions over iroh P2P (QUIC, end-to-end encrypted, NAT hole-punching
//! with free public relays as fallback). No server to run, no port forwarding.
//!
//! Room addressing: the host's iroh identity is *derived* from
//! `username + password` with a KDF. A viewer who types the same username (and
//! password, if set) computes the same public key and dials it directly through
//! iroh discovery. A wrong password produces a different key — the connection
//! is cryptographically impossible, and the password itself never leaves the
//! machine.
//!
//! Trade-off (documented in SECURITY.md): for password-less public rooms the
//! room key is derivable from the public username, so identity is only as
//! secret as the name. Use a password for private sessions.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use iroh::endpoint::presets;
use iroh::{Endpoint, EndpointAddr, EndpointId, SecretKey};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::bus::IntensityBus;

pub const ALPN: &[u8] = b"vibeloop/1";
const KDF_CONTEXT: &str = "vibeloop.app 2026-07 room key v1";
const PING_INTERVAL: Duration = Duration::from_secs(5);
/// If nothing (not even a ping) arrives for this long, the link is dead.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(15);

/// Events surfaced to the UI layer.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Host is bound, published, and reachable under this room name.
    HostReady { room: String },
    /// Viewer count changed (host side).
    Viewers(usize),
    /// Connected to the host (viewer side).
    Connected { room: String },
    /// Lost the host; retrying. Intensity has been zeroed.
    Reconnecting { detail: String },
    /// Received intensity (viewer side) — already applied to the bus.
    Intensity(f64),
    /// Human-readable log line.
    Log(String),
    /// Session ended for good (only after `stop`, or a fatal setup error).
    Ended { reason: String },
}

/// Derives the deterministic room secret from username (+ optional password).
pub fn derive_room_secret(username: &str, password: &str) -> SecretKey {
    let mut hasher = blake3::Hasher::new_derive_key(KDF_CONTEXT);
    hasher.update(normalize_room(username).as_bytes());
    hasher.update(&[0x1f]);
    hasher.update(password.as_bytes());
    SecretKey::from_bytes(hasher.finalize().as_bytes())
}

/// Room names are case-insensitive and whitespace-trimmed so "Patrix" and
/// " patrix " land in the same room.
pub fn normalize_room(username: &str) -> String {
    username.trim().to_lowercase()
}

/// Basic sanity check with a human-readable error, done before anything binds.
pub fn validate_username(username: &str) -> Result<String, String> {
    let name = username.trim();
    if name.is_empty() {
        return Err("Please enter a username first.".into());
    }
    if name.len() > 32 {
        return Err("Username is too long (max 32 characters).".into());
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err("Username can only contain letters, numbers, _ - and .".into());
    }
    Ok(name.to_string())
}

/// A running host or viewer session. Dropping/stopping zeroes intensity.
pub struct Session {
    cancel: CancellationToken,
}

impl Session {
    pub fn stop(&self) {
        self.cancel.cancel();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

// ─── Host ────────────────────────────────────────────────────────────────────

/// Starts hosting a room. Broadcasts every intensity change from the bus to all
/// connected viewers until stopped.
pub async fn host(
    username: &str,
    password: &str,
    bus: &IntensityBus,
    events: mpsc::UnboundedSender<SessionEvent>,
) -> Result<Session> {
    let room = normalize_room(username);
    let secret = derive_room_secret(username, password);
    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(secret)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .context("could not start P2P networking — check your internet connection")?;

    let cancel = CancellationToken::new();
    let viewers = Arc::new(AtomicUsize::new(0));
    let _ = events.send(SessionEvent::HostReady { room: room.clone() });
    let _ = events.send(SessionEvent::Log(format!(
        "Hosting room '{room}' — waiting for viewers"
    )));

    let rx = bus.subscribe();
    let accept_cancel = cancel.clone();
    let accept_events = events.clone();
    tokio::spawn(async move {
        loop {
            let incoming = tokio::select! {
                _ = accept_cancel.cancelled() => break,
                incoming = endpoint.accept() => match incoming {
                    Some(i) => i,
                    None => break,
                },
            };
            let conn = match incoming.await {
                Ok(c) => c,
                Err(e) => {
                    let _ = accept_events.send(SessionEvent::Log(format!(
                        "A viewer failed to connect: {e}"
                    )));
                    continue;
                }
            };
            let viewers = viewers.clone();
            let events = accept_events.clone();
            let cancel = accept_cancel.clone();
            let mut rx = rx.clone();
            tokio::spawn(async move {
                let peer = conn.remote_id().fmt_short().to_string();
                let count = viewers.fetch_add(1, Ordering::SeqCst) + 1;
                let _ = events.send(SessionEvent::Viewers(count));
                let _ = events.send(SessionEvent::Log(format!("Viewer {peer} joined")));

                let result: Result<()> = async {
                    let (mut send, _recv) = conn.accept_bi().await?;
                    let mut ping = tokio::time::interval(PING_INTERVAL);
                    loop {
                        let line = tokio::select! {
                            _ = cancel.cancelled() => break,
                            _ = ping.tick() => format!("{{\"t\":\"i\",\"v\":{}}}\n", *rx.borrow()),
                            changed = rx.changed() => {
                                if changed.is_err() { break; }
                                format!("{{\"t\":\"i\",\"v\":{}}}\n", *rx.borrow_and_update())
                            }
                        };
                        send.write_all(line.as_bytes()).await?;
                    }
                    // Polite goodbye so the viewer shows "host ended" not "lost".
                    let _ = send.write_all(b"{\"t\":\"bye\"}\n").await;
                    let _ = send.finish();
                    conn.close(0u32.into(), b"session ended");
                    Ok(())
                }
                .await;

                let count = viewers.fetch_sub(1, Ordering::SeqCst) - 1;
                let _ = events.send(SessionEvent::Viewers(count));
                let reason = match result {
                    Ok(()) => "session ended".to_string(),
                    Err(e) => format!("{e}"),
                };
                let _ = events.send(SessionEvent::Log(format!(
                    "Viewer {peer} left ({reason})"
                )));
            });
        }
        endpoint.close().await;
    });

    Ok(Session { cancel })
}

// ─── Viewer ──────────────────────────────────────────────────────────────────

/// Joins a room as a viewer. Received intensity is written into the bus.
/// Auto-reconnects with backoff until stopped; zeroes the bus on any loss.
pub async fn join(
    username: &str,
    password: &str,
    bus: &IntensityBus,
    events: mpsc::UnboundedSender<SessionEvent>,
) -> Result<Session> {
    let room = normalize_room(username);
    let has_password = !password.is_empty();
    let host_id: EndpointId = derive_room_secret(username, password).public();
    let endpoint = Endpoint::builder(presets::N0)
        .alpns(vec![])
        .bind()
        .await
        .context("could not start P2P networking — check your internet connection")?;

    let cancel = CancellationToken::new();
    let bus = bus.clone();
    let task_cancel = cancel.clone();
    tokio::spawn(async move {
        let mut attempt: u32 = 0;
        loop {
            if task_cancel.is_cancelled() {
                break;
            }
            attempt += 1;
            let connect = tokio::select! {
                _ = task_cancel.cancelled() => break,
                c = endpoint.connect(EndpointAddr::from(host_id), ALPN) => c,
            };
            let conn = match connect {
                Ok(c) => c,
                Err(e) => {
                    bus.kill();
                    let hint = if has_password {
                        "Check the username and password, and that the host has started."
                    } else {
                        "Check the username, and that the host has started."
                    };
                    let detail = if attempt == 1 {
                        format!("Can't find room '{room}'. {hint}")
                    } else {
                        format!("Still looking for room '{room}'… ({e})")
                    };
                    let _ = events.send(SessionEvent::Reconnecting { detail });
                    backoff(&task_cancel, attempt).await;
                    continue;
                }
            };

            let _ = events.send(SessionEvent::Connected { room: room.clone() });
            let _ = events.send(SessionEvent::Log(format!("Connected to room '{room}'")));
            attempt = 0;

            let mut ended_by_host = false;
            let stream_result: Result<()> = async {
                let (send, recv) = conn.open_bi().await?;
                drop(send); // viewer never talks; intensity flows one way
                let mut lines = BufReader::new(recv).lines();
                loop {
                    let line = tokio::select! {
                        _ = task_cancel.cancelled() => return Ok(()),
                        line = tokio::time::timeout(CLIENT_TIMEOUT, lines.next_line()) => {
                            match line {
                                Err(_) => anyhow::bail!("no data from host for {}s", CLIENT_TIMEOUT.as_secs()),
                                Ok(l) => l?,
                            }
                        }
                    };
                    let Some(line) = line else { break };
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                        match v.get("t").and_then(|t| t.as_str()) {
                            Some("i") => {
                                let level =
                                    v.get("v").and_then(|x| x.as_f64()).unwrap_or(0.0);
                                bus.set_base(level.clamp(0.0, 1.0));
                                let _ = events.send(SessionEvent::Intensity(level));
                            }
                            Some("bye") => {
                                ended_by_host = true;
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                }
                Ok(())
            }
            .await;

            bus.kill();
            if task_cancel.is_cancelled() {
                break;
            }
            if ended_by_host {
                let _ = events.send(SessionEvent::Ended {
                    reason: "The host ended the session.".into(),
                });
                break;
            }
            let detail = match stream_result {
                Ok(()) => "Lost the host — reconnecting…".to_string(),
                Err(e) => format!("Lost the host ({e}) — reconnecting…"),
            };
            let _ = events.send(SessionEvent::Reconnecting { detail });
            backoff(&task_cancel, 1).await;
        }
        bus.kill();
        endpoint.close().await;
    });

    Ok(Session { cancel })
}

async fn backoff(cancel: &CancellationToken, attempt: u32) {
    let secs = (2u64.saturating_pow(attempt.min(4))).min(10);
    tokio::select! {
        _ = cancel.cancelled() => {}
        _ = tokio::time::sleep(Duration::from_secs(secs)) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_credentials_same_room() {
        let a = derive_room_secret("Patrix", "pw");
        let b = derive_room_secret("  patrix ", "pw");
        assert_eq!(a.public(), b.public());
    }

    #[test]
    fn password_changes_room() {
        let a = derive_room_secret("patrix", "");
        let b = derive_room_secret("patrix", "secret");
        assert_ne!(a.public(), b.public());
    }

    #[test]
    fn username_rules() {
        assert!(validate_username("Cool_Name-99").is_ok());
        assert!(validate_username("").is_err());
        assert!(validate_username("has space").is_err());
    }
}
