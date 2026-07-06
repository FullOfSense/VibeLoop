//! End-to-end tests for the three new source kinds, each against a fake game
//! on localhost: an HTTP server for `poll`, a GSI-style POSTer for `listen`,
//! and a raw OSC sender for `osc`. Mods drive the bus exactly as in production.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use vibeloop_core::modengine::{run_mod, ModEvent};
use vibeloop_core::IntensityBus;

fn write_mod(name: &str, body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("vibeloop-srcs-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.lua"));
    std::fs::write(&path, body).unwrap();
    path
}

/// Waits until the bus reports at least `level`, or panics after `secs`.
async fn expect_level(bus: &IntensityBus, level: f64, secs: u64) {
    let mut rx = bus.subscribe();
    tokio::time::timeout(Duration::from_secs(secs), async {
        loop {
            if *rx.borrow() >= level {
                break;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .unwrap_or_else(|_| panic!("bus never reached {level}"));
}

#[tokio::test]
async fn poll_source_delivers_json_bodies() {
    // Fake game API: every GET returns a small JSON state.
    let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = server.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = server.accept().await else { break };
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                loop {
                    let Ok(n) = sock.read(&mut buf).await else { return };
                    if n == 0 {
                        return;
                    }
                    let body = r#"{"health":37}"#;
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    if sock.write_all(resp.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });

    let mod_path = write_mod(
        "poll",
        &format!(
            r#"
sources = {{ {{ id = "api", url = "http://127.0.0.1:{port}/state", interval = 0.1 }} }}
function on_message(source, data)
  if data.health == 37 then vibe.set(0.6) end
end
"#
        ),
    );
    let bus = IntensityBus::new();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let running = run_mod(&mod_path, &bus, tx).await.unwrap();
    expect_level(&bus, 0.59, 5).await;

    // The source must have announced itself up.
    let mut saw_up = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, ModEvent::SourceUp { .. }) {
            saw_up = true;
        }
    }
    assert!(saw_up, "poll source never reported SourceUp");
    running.stop();
    bus.shutdown();
}

#[tokio::test]
async fn listen_source_accepts_gsi_posts() {
    // Bind port 0 trick isn't possible (the mod declares the port), so pick
    // an ephemeral-range port unlikely to collide.
    let port = 38213u16;
    let mod_path = write_mod(
        "listen",
        &format!(
            r#"
sources = {{ {{ id = "gsi", type = "listen", port = {port} }} }}
function on_message(source, data)
  if data.player and data.player.health == 12 then vibe.set(0.8) end
end
"#
        ),
    );
    let bus = IntensityBus::new();
    let (tx, _rx) = mpsc::unbounded_channel();
    let running = run_mod(&mod_path, &bus, tx).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await; // let it bind

    // Fake CS2: POST a GSI payload, expect 200, keep the connection open.
    let mut sock = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let body = r#"{"player":{"health":12}}"#;
    let req = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    sock.write_all(req.as_bytes()).await.unwrap();
    let mut resp = [0u8; 1024];
    let n = tokio::time::timeout(Duration::from_secs(2), sock.read(&mut resp))
        .await
        .unwrap()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&resp[..n]).starts_with("HTTP/1.1 200"),
        "listener must answer 200 OK"
    );
    expect_level(&bus, 0.79, 5).await;
    running.stop();
    bus.shutdown();
}

#[tokio::test]
async fn file_source_tails_without_replaying_history() {
    let dir = std::env::temp_dir().join(format!("vibeloop-tail-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let log = dir.join("game.log");
    // Pre-existing content must NOT be replayed.
    std::fs::write(&log, "HIT\nHIT\n").unwrap();

    let mod_path = write_mod(
        "tail",
        &format!(
            r#"
sources = {{ {{ id = "log", type = "file", path = "{}" }} }}
hits = 0
function on_message(source, data)
  if data.line == "HIT" then
    hits = hits + 1
    vibe.set(0.4 + hits * 0.1)   -- 1st HIT → 0.5, 2nd HIT → 0.6, …
  elseif data.v == 9 then
    vibe.set(0.9)
  end
end
"#,
            log.display()
        ),
    );
    let bus = IntensityBus::new();
    let (tx, _rx) = mpsc::unbounded_channel();
    let running = run_mod(&mod_path, &bus, tx).await.unwrap();
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(*bus.subscribe().borrow(), 0.0, "history was replayed");

    // Plain line → wrapped as {"line": ...}.
    let mut f = std::fs::OpenOptions::new().append(true).open(&log).unwrap();
    use std::io::Write;
    writeln!(f, "HIT").unwrap();
    f.sync_all().unwrap();
    expect_level(&bus, 0.49, 5).await;

    // JSON line → passed through as-is.
    writeln!(f, r#"{{"v":9}}"#).unwrap();
    f.sync_all().unwrap();
    expect_level(&bus, 0.89, 5).await;
    drop(f);

    // Truncation (new game session) must not kill the tail. The second HIT
    // sets exactly 0.6 — a level nothing else in this test produces — so
    // seeing the bus settle there proves the post-truncation line was read.
    std::fs::write(&log, "").unwrap();
    tokio::time::sleep(Duration::from_millis(600)).await;
    let mut f = std::fs::OpenOptions::new().append(true).open(&log).unwrap();
    writeln!(f, "HIT").unwrap();
    f.sync_all().unwrap();
    let mut rx = bus.subscribe();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if (*rx.borrow() - 0.6).abs() < 0.02 {
                break;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .expect("tail died after truncation — second HIT never arrived");

    running.stop();
    bus.shutdown();
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn osc_source_decodes_vrchat_parameters() {
    let port = 38214u16;
    let mod_path = write_mod(
        "osc",
        &format!(
            r#"
sources = {{ {{ id = "vrc", type = "osc", port = {port} }} }}
function on_message(source, data)
  if data.addr == "/avatar/parameters/VibeLoop" then vibe.set(data.args[1]) end
end
"#
        ),
    );
    let bus = IntensityBus::new();
    let (tx, _rx) = mpsc::unbounded_channel();
    let running = run_mod(&mod_path, &bus, tx).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await; // let it bind

    // Fake VRChat: send the parameter as a plain OSC message over UDP.
    let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let packet = rosc::encoder::encode(&rosc::OscPacket::Message(rosc::OscMessage {
        addr: "/avatar/parameters/VibeLoop".into(),
        args: vec![rosc::OscType::Float(0.7)],
    }))
    .unwrap();
    // A few sends in case the first races the bind.
    for _ in 0..5 {
        sock.send_to(&packet, ("127.0.0.1", port)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    expect_level(&bus, 0.69, 5).await;
    running.stop();
    bus.shutdown();
}
