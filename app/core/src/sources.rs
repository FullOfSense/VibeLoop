//! Source connectors: how game data reaches a running mod.
//!
//! A mod declares *what* to listen to; this module owns *how* — connecting,
//! reconnecting, reporting up/down to the UI, and delivering every payload as
//! a `(source_id, json_text)` message. Four kinds:
//!
//! - `ws` — connect to a WebSocket and forward text frames (osu!/tosu,
//!   Beat Saber DataPuller). Inferred from `ws://`/`wss://` URLs.
//! - `poll` — GET an HTTP(S) URL on an interval and forward the body
//!   (League of Legends Live Client API, War Thunder localhost:8111).
//!   Inferred from `http://`/`https://` URLs. `insecure = true` accepts
//!   self-signed certificates but only for loopback hosts (League's API).
//! - `listen` — accept HTTP POSTs on a local port and forward each body
//!   (Counter-Strike 2 Game State Integration pushes JSON to us).
//! - `osc` — receive OSC packets on a local UDP port (VRChat avatar
//!   parameters), forwarded as `{"addr": "/avatar/...", "args": [...]}`.
//! - `file` — tail a file the game (or a tiny in-game bridge) writes to:
//!   Factorio's script-output, TF2's console.log, Isaac's log.txt. New lines
//!   are forwarded as-is when they are JSON, else wrapped as `{"line": "…"}`.
//!   Tailing starts at the end of the file — history is never replayed.
//!
//! Listeners bind 127.0.0.1 only: nothing on the network can feed a mod.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::modengine::ModEvent;

const RECONNECT_DELAY: Duration = Duration::from_secs(3);
/// Poll/listen payloads above this are dropped — no game state is this big.
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
const MAX_HEADER_BYTES: usize = 16 * 1024;
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// One parsed `sources = { ... }` entry.
#[derive(Debug, Clone)]
pub struct SourceSpec {
    pub id: String,
    pub kind: SourceKind,
}

#[derive(Debug, Clone)]
pub enum SourceKind {
    Ws { url: String },
    Poll { url: String, interval: Duration, insecure: bool },
    HttpListen { port: u16 },
    Osc { port: u16 },
    /// Candidate paths (OS-dependent install locations); the first one that
    /// exists gets tailed.
    File { paths: Vec<String> },
}

/// Parses one Lua source entry. `type` may be omitted for `ws`/`poll` —
/// it's inferred from the URL scheme.
pub fn parse_spec(entry: &mlua::Table) -> Result<SourceSpec> {
    let id: String = entry.get("id").context("source needs an `id`")?;
    let explicit: Option<String> = entry.get("type").ok().flatten();
    let url: Option<String> = entry.get("url").ok().flatten();

    let inferred = match (&explicit, &url) {
        (Some(t), _) => t.clone(),
        (None, Some(u)) if u.starts_with("ws://") || u.starts_with("wss://") => "ws".into(),
        (None, Some(u)) if u.starts_with("http://") || u.starts_with("https://") => "poll".into(),
        (None, Some(u)) => bail!("source '{id}': can't infer type from url '{u}' — set `type`"),
        (None, None) => bail!("source '{id}': needs a `url` or a `type`"),
    };

    let kind = match inferred.as_str() {
        "ws" => SourceKind::Ws {
            url: url.with_context(|| format!("source '{id}': ws source needs a `url`"))?,
        },
        "poll" => {
            let url =
                url.with_context(|| format!("source '{id}': poll source needs a `url`"))?;
            let interval: f64 = entry.get("interval").ok().flatten().unwrap_or(0.25);
            let interval = if interval.is_finite() {
                Duration::from_secs_f64(interval.clamp(0.05, 30.0))
            } else {
                Duration::from_millis(250)
            };
            let insecure: bool = entry.get("insecure").ok().flatten().unwrap_or(false);
            if insecure && !is_loopback_url(&url) {
                bail!(
                    "source '{id}': `insecure = true` is only allowed for \
                     127.0.0.1/localhost URLs"
                );
            }
            SourceKind::Poll { url, interval, insecure }
        }
        "listen" => SourceKind::HttpListen {
            port: entry
                .get::<Option<u16>>("port")
                .ok()
                .flatten()
                .with_context(|| format!("source '{id}': listen source needs a `port`"))?,
        },
        "osc" => SourceKind::Osc {
            port: entry
                .get::<Option<u16>>("port")
                .ok()
                .flatten()
                .with_context(|| format!("source '{id}': osc source needs a `port`"))?,
        },
        "file" => {
            let mut paths: Vec<String> = Vec::new();
            if let Ok(Some(p)) = entry.get::<Option<String>>("path") {
                paths.push(p);
            }
            if let Ok(Some(list)) = entry.get::<Option<mlua::Table>>("paths") {
                for p in list.sequence_values::<String>() {
                    paths.push(p?);
                }
            }
            if paths.is_empty() {
                bail!("source '{id}': file source needs a `path` or a `paths` list");
            }
            SourceKind::File { paths }
        }
        other => bail!(
            "source '{id}': unknown type '{other}' (expected ws, poll, listen, osc or file)"
        ),
    };
    Ok(SourceSpec { id, kind })
}

fn is_loopback_url(url: &str) -> bool {
    url::host(url).is_some_and(|h| {
        matches!(h.as_str(), "127.0.0.1" | "localhost" | "[::1]" | "::1")
    })
}

/// Tiny scheme-agnostic host extraction — avoids pulling in a URL crate.
mod url {
    pub fn host(url: &str) -> Option<String> {
        let rest = url.split_once("://")?.1;
        let authority = rest.split(['/', '?', '#']).next()?;
        let authority = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        // Keep IPv6 brackets intact; otherwise strip a :port suffix.
        if authority.starts_with('[') {
            return Some(authority.split(']').next()?.trim_start_matches('[').to_string())
                .map(|h| format!("[{h}]"));
        }
        Some(authority.split(':').next()?.to_string())
    }
}

/// Spawns the connector task for one source.
pub fn spawn(
    spec: SourceSpec,
    msg_tx: mpsc::Sender<(String, String)>,
    events: mpsc::UnboundedSender<ModEvent>,
    cancel: CancellationToken,
) {
    match spec.kind {
        SourceKind::Ws { url } => {
            tokio::spawn(ws_loop(spec.id, url, msg_tx, events, cancel));
        }
        SourceKind::Poll { url, interval, insecure } => {
            tokio::spawn(poll_loop(spec.id, url, interval, insecure, msg_tx, events, cancel));
        }
        SourceKind::HttpListen { port } => {
            tokio::spawn(http_listen_loop(spec.id, port, msg_tx, events, cancel));
        }
        SourceKind::Osc { port } => {
            tokio::spawn(osc_loop(spec.id, port, msg_tx, events, cancel));
        }
        SourceKind::File { paths } => {
            tokio::spawn(file_loop(spec.id, paths, msg_tx, events, cancel));
        }
    }
}

/// Expands `~` and `${VAR}` in a declared path. Unset variables leave the
/// candidate unusable, which is fine — it simply never exists.
fn expand_path(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let mut rest = raw;
    if let Some(tail) = rest.strip_prefix("~") {
        out.push_str(&home);
        rest = tail;
    }
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                out.push_str(&std::env::var(&after[..end]).unwrap_or_default());
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Tails the first existing candidate path. Starts at the end of the file
/// (no history), survives truncation (new game session) and the file
/// disappearing (game closed). Lines that already are JSON pass through;
/// anything else is wrapped as `{"line": "…"}`.
async fn file_loop(
    id: String,
    paths: Vec<String>,
    msg_tx: mpsc::Sender<(String, String)>,
    events: mpsc::UnboundedSender<ModEvent>,
    cancel: CancellationToken,
) {
    use tokio::io::AsyncSeekExt;

    let candidates: Vec<String> = paths.iter().map(|p| expand_path(p)).collect();
    let mut announced_down = false;
    'outer: loop {
        if cancel.is_cancelled() {
            return;
        }
        // Wait for any candidate to exist.
        let path = loop {
            if let Some(p) = candidates.iter().find(|p| std::path::Path::new(p).is_file()) {
                break p.clone();
            }
            if !announced_down {
                announced_down = true;
                let _ = events.send(ModEvent::SourceDown {
                    source: id.clone(),
                    detail: "log file not found yet — waiting for the game".into(),
                });
            }
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            }
        };

        let Ok(mut file) = tokio::fs::File::open(&path).await else {
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(RECONNECT_DELAY) => {}
            }
            continue;
        };
        // Start at the end: what happened before the mod started is history.
        let mut pos = file.seek(std::io::SeekFrom::End(0)).await.unwrap_or(0);
        let _ = events.send(ModEvent::SourceUp { source: id.clone() });
        announced_down = false;
        let mut partial = Vec::new();
        let mut chunk = vec![0u8; 64 * 1024];

        loop {
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(Duration::from_millis(250)) => {}
            }
            let len = match tokio::fs::metadata(&path).await {
                Ok(m) => m.len(),
                Err(_) => {
                    // File went away (game closed / log rotated).
                    let _ = events.send(ModEvent::SourceDown {
                        source: id.clone(),
                        detail: "log file disappeared — waiting for the game".into(),
                    });
                    announced_down = true;
                    continue 'outer;
                }
            };
            if len < pos {
                // Truncated: a new session started writing from the top.
                pos = 0;
                partial.clear();
                if file.seek(std::io::SeekFrom::Start(0)).await.is_err() {
                    continue 'outer;
                }
            }
            while pos < len {
                let n = match file.read(&mut chunk).await {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(_) => continue 'outer,
                };
                pos += n as u64;
                partial.extend_from_slice(&chunk[..n]);
                // Guard against a game writing one enormous line.
                if partial.len() > MAX_BODY_BYTES {
                    partial.clear();
                }
                while let Some(nl) = partial.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = partial.drain(..=nl).collect();
                    let line = String::from_utf8_lossy(&line).trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    let payload = if serde_json::from_str::<serde_json::Value>(&line).is_ok() {
                        line
                    } else {
                        serde_json::json!({ "line": line }).to_string()
                    };
                    let _ = msg_tx.try_send((id.clone(), payload));
                }
            }
        }
    }
}

/// Connects to one WebSocket source, forwards text frames, reconnects forever.
/// When several frames are queued we keep only the freshest (game state
/// snapshots supersede each other; stale ones just add latency).
async fn ws_loop(
    id: String,
    url: String,
    msg_tx: mpsc::Sender<(String, String)>,
    events: mpsc::UnboundedSender<ModEvent>,
    cancel: CancellationToken,
) {
    let mut announced_down = false;
    loop {
        if cancel.is_cancelled() {
            return;
        }
        let connect = tokio::select! {
            _ = cancel.cancelled() => return,
            c = tokio_tungstenite::connect_async(&url) => c,
        };
        match connect {
            Ok((mut ws, _)) => {
                let _ = events.send(ModEvent::SourceUp { source: id.clone() });
                loop {
                    let frame = tokio::select! {
                        _ = cancel.cancelled() => return,
                        f = ws.next() => f,
                    };
                    match frame {
                        Some(Ok(msg)) if msg.is_text() => {
                            let text = msg.into_text().map(|t| t.to_string()).unwrap_or_default();
                            // Drop the queued frame if the mod is behind; the
                            // next snapshot carries the full state anyway.
                            let _ = msg_tx.try_send((id.clone(), text));
                        }
                        Some(Ok(_)) => {}
                        Some(Err(_)) | None => break,
                    }
                }
                let _ = events.send(ModEvent::SourceDown {
                    source: id.clone(),
                    detail: "connection lost — retrying".into(),
                });
                announced_down = true;
            }
            Err(e) => {
                if !announced_down {
                    let _ = events.send(ModEvent::SourceDown {
                        source: id.clone(),
                        detail: format!("can't reach {url} ({e}) — retrying"),
                    });
                    announced_down = true;
                }
            }
        }
        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep(RECONNECT_DELAY) => {}
        }
    }
}

/// GETs `url` every `interval`, forwarding each successful body. Failures
/// switch to the slower reconnect cadence until the game answers again.
async fn poll_loop(
    id: String,
    url: String,
    interval: Duration,
    insecure: bool,
    msg_tx: mpsc::Sender<(String, String)>,
    events: mpsc::UnboundedSender<ModEvent>,
    cancel: CancellationToken,
) {
    let client = match reqwest::Client::builder()
        .danger_accept_invalid_certs(insecure)
        .timeout(HTTP_REQUEST_TIMEOUT)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = events.send(ModEvent::SourceDown {
                source: id,
                detail: format!("could not build HTTP client: {e}"),
            });
            return;
        }
    };
    let mut up = false;
    let mut announced_down = false;
    loop {
        if cancel.is_cancelled() {
            return;
        }
        let result = tokio::select! {
            _ = cancel.cancelled() => return,
            r = fetch_body(&client, &url) => r,
        };
        match result {
            Ok(body) => {
                if !up {
                    up = true;
                    announced_down = false;
                    let _ = events.send(ModEvent::SourceUp { source: id.clone() });
                }
                let _ = msg_tx.try_send((id.clone(), body));
            }
            Err(e) => {
                if !announced_down {
                    announced_down = true;
                    let detail = if up {
                        "connection lost — retrying".to_string()
                    } else {
                        format!("can't reach {url} ({e}) — retrying")
                    };
                    let _ = events.send(ModEvent::SourceDown { source: id.clone(), detail });
                }
                up = false;
            }
        }
        let delay = if up { interval } else { RECONNECT_DELAY };
        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep(delay) => {}
        }
    }
}

async fn fetch_body(client: &reqwest::Client, url: &str) -> Result<String> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        bail!("HTTP {}", resp.status());
    }
    if resp.content_length().unwrap_or(0) as usize > MAX_BODY_BYTES {
        bail!("response too large");
    }
    let body = resp.text().await?;
    if body.len() > MAX_BODY_BYTES {
        bail!("response too large");
    }
    Ok(body)
}

/// Shared "is the game still talking to us" bookkeeping for push sources.
struct Presence {
    up: AtomicBool,
    last_data: Mutex<Instant>,
}

impl Presence {
    fn new() -> Arc<Self> {
        Arc::new(Self { up: AtomicBool::new(false), last_data: Mutex::new(Instant::now()) })
    }

    fn data_arrived(&self, id: &str, events: &mpsc::UnboundedSender<ModEvent>) {
        *self.last_data.lock().expect("presence lock") = Instant::now();
        if !self.up.swap(true, Ordering::SeqCst) {
            let _ = events.send(ModEvent::SourceUp { source: id.to_string() });
        }
    }

    /// Ticks every few seconds; drops the source to "down" after `timeout`
    /// without data so the UI shows the game went away.
    async fn watchdog(
        self: Arc<Self>,
        id: String,
        timeout: Duration,
        events: mpsc::UnboundedSender<ModEvent>,
        cancel: CancellationToken,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = interval.tick() => {}
            }
            let idle = self.last_data.lock().expect("presence lock").elapsed();
            if idle > timeout && self.up.swap(false, Ordering::SeqCst) {
                let _ = events.send(ModEvent::SourceDown {
                    source: id.clone(),
                    detail: format!(
                        "no data for {}s — waiting for the game",
                        idle.as_secs()
                    ),
                });
            }
        }
    }
}

/// Accepts HTTP POSTs on 127.0.0.1:`port` (CS2 Game State Integration style)
/// and forwards each request body.
async fn http_listen_loop(
    id: String,
    port: u16,
    msg_tx: mpsc::Sender<(String, String)>,
    events: mpsc::UnboundedSender<ModEvent>,
    cancel: CancellationToken,
) {
    let listener = loop {
        if cancel.is_cancelled() {
            return;
        }
        match TcpListener::bind(("127.0.0.1", port)).await {
            Ok(l) => break l,
            Err(e) => {
                let _ = events.send(ModEvent::SourceDown {
                    source: id.clone(),
                    detail: format!("can't listen on port {port} ({e}) — retrying"),
                });
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(RECONNECT_DELAY) => {}
                }
            }
        }
    };
    let _ = events.send(ModEvent::SourceDown {
        source: id.clone(),
        detail: format!("listening on 127.0.0.1:{port} — waiting for the game"),
    });

    let presence = Presence::new();
    tokio::spawn(presence.clone().watchdog(
        id.clone(),
        Duration::from_secs(30),
        events.clone(),
        cancel.clone(),
    ));

    loop {
        let accepted = tokio::select! {
            _ = cancel.cancelled() => return,
            a = listener.accept() => a,
        };
        let Ok((stream, _addr)) = accepted else { continue };
        let id = id.clone();
        let msg_tx = msg_tx.clone();
        let events = events.clone();
        let cancel = cancel.clone();
        let presence = presence.clone();
        tokio::spawn(async move {
            let _ = serve_http_conn(stream, &id, &msg_tx, &events, &presence, cancel).await;
        });
    }
}

/// Handles sequential requests on one connection (games keep it alive).
async fn serve_http_conn(
    mut stream: TcpStream,
    id: &str,
    msg_tx: &mpsc::Sender<(String, String)>,
    events: &mpsc::UnboundedSender<ModEvent>,
    presence: &Presence,
    cancel: CancellationToken,
) -> Result<()> {
    let mut buf: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut chunk = [0u8; 8 * 1024];
    loop {
        // Read until we hold the full header block.
        let header_end = loop {
            if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
                break pos + 4;
            }
            if buf.len() > MAX_HEADER_BYTES {
                bail!("header too large");
            }
            let n = tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                n = stream.read(&mut chunk) => n?,
            };
            if n == 0 {
                return Ok(()); // client closed between requests
            }
            buf.extend_from_slice(&chunk[..n]);
        };

        let headers = String::from_utf8_lossy(&buf[..header_end]).to_string();
        let content_length = headers
            .lines()
            .find_map(|l| {
                let (k, v) = l.split_once(':')?;
                k.trim().eq_ignore_ascii_case("content-length").then(|| v.trim().parse::<usize>().ok())?
            })
            .unwrap_or(0);
        if content_length > MAX_BODY_BYTES {
            bail!("body too large");
        }

        while buf.len() < header_end + content_length {
            let n = tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                n = stream.read(&mut chunk) => n?,
            };
            if n == 0 {
                bail!("connection closed mid-body");
            }
            buf.extend_from_slice(&chunk[..n]);
        }

        let body = String::from_utf8_lossy(&buf[header_end..header_end + content_length]).to_string();
        buf.drain(..header_end + content_length);

        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .await?;

        if !body.trim().is_empty() {
            presence.data_arrived(id, events);
            let _ = msg_tx.try_send((id.to_string(), body));
        }
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Receives OSC packets on 127.0.0.1:`port` (VRChat sends avatar parameters
/// here). Every OSC message becomes `{"addr": "...", "args": [...]}`.
async fn osc_loop(
    id: String,
    port: u16,
    msg_tx: mpsc::Sender<(String, String)>,
    events: mpsc::UnboundedSender<ModEvent>,
    cancel: CancellationToken,
) {
    let socket = loop {
        if cancel.is_cancelled() {
            return;
        }
        match UdpSocket::bind(("127.0.0.1", port)).await {
            Ok(s) => break s,
            Err(e) => {
                let _ = events.send(ModEvent::SourceDown {
                    source: id.clone(),
                    detail: format!("can't listen on UDP port {port} ({e}) — retrying"),
                });
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(RECONNECT_DELAY) => {}
                }
            }
        }
    };
    let _ = events.send(ModEvent::SourceDown {
        source: id.clone(),
        detail: format!("listening on UDP 127.0.0.1:{port} — waiting for the game"),
    });

    let presence = Presence::new();
    tokio::spawn(presence.clone().watchdog(
        id.clone(),
        Duration::from_secs(60),
        events.clone(),
        cancel.clone(),
    ));

    let mut buf = [0u8; 64 * 1024];
    loop {
        let received = tokio::select! {
            _ = cancel.cancelled() => return,
            r = socket.recv_from(&mut buf) => r,
        };
        let Ok((n, _addr)) = received else { continue };
        let Ok((_, packet)) = rosc::decoder::decode_udp(&buf[..n]) else { continue };
        let mut messages = Vec::new();
        flatten_osc(packet, &mut messages);
        for msg in messages {
            presence.data_arrived(&id, &events);
            let _ = msg_tx.try_send((id.clone(), msg));
        }
    }
}

fn flatten_osc(packet: rosc::OscPacket, out: &mut Vec<String>) {
    match packet {
        rosc::OscPacket::Message(m) => {
            let args: Vec<serde_json::Value> = m
                .args
                .into_iter()
                .filter_map(|a| match a {
                    rosc::OscType::Float(f) if f.is_finite() => Some(f.into()),
                    rosc::OscType::Double(d) if d.is_finite() => Some(d.into()),
                    rosc::OscType::Int(i) => Some(i.into()),
                    rosc::OscType::Long(l) => Some(l.into()),
                    rosc::OscType::Bool(b) => Some(b.into()),
                    rosc::OscType::String(s) => Some(s.into()),
                    _ => None,
                })
                .collect();
            let json = serde_json::json!({ "addr": m.addr, "args": args });
            out.push(json.to_string());
        }
        rosc::OscPacket::Bundle(b) => {
            for inner in b.content {
                flatten_osc(inner, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lua_entry(code: &str) -> mlua::Table {
        let lua = mlua::Lua::new();
        let table: mlua::Table = lua.load(code).eval().unwrap();
        // Leak the Lua state so the table stays valid for the test body.
        std::mem::forget(lua);
        table
    }

    #[test]
    fn infers_types_from_url_scheme() {
        let ws = parse_spec(&lua_entry("{ id = 'a', url = 'ws://127.0.0.1:1/x' }")).unwrap();
        assert!(matches!(ws.kind, SourceKind::Ws { .. }));
        let poll = parse_spec(&lua_entry("{ id = 'b', url = 'http://127.0.0.1:1/x' }")).unwrap();
        assert!(matches!(poll.kind, SourceKind::Poll { .. }));
    }

    #[test]
    fn insecure_requires_loopback() {
        let bad = parse_spec(&lua_entry(
            "{ id = 'a', url = 'https://example.com/x', insecure = true }",
        ));
        assert!(bad.is_err());
        let ok = parse_spec(&lua_entry(
            "{ id = 'a', url = 'https://127.0.0.1:2999/x', insecure = true }",
        ));
        assert!(ok.is_ok());
    }

    #[test]
    fn listen_and_osc_need_ports() {
        assert!(parse_spec(&lua_entry("{ id = 'a', type = 'listen' }")).is_err());
        assert!(parse_spec(&lua_entry("{ id = 'a', type = 'osc', port = 9001 }")).is_ok());
        assert!(parse_spec(&lua_entry("{ id = 'a', type = 'nope' }")).is_err());
    }

    #[test]
    fn host_extraction() {
        assert_eq!(url::host("http://localhost:8111/state").as_deref(), Some("localhost"));
        assert_eq!(url::host("https://127.0.0.1:2999/a?b#c").as_deref(), Some("127.0.0.1"));
        assert_eq!(url::host("ws://[::1]:9001/x").as_deref(), Some("[::1]"));
        assert_eq!(url::host("nonsense").as_deref(), None);
    }
}
