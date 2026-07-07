//! Logic tests for the Tier B (file-bridge) mods: each mod's `on_message`
//! is called with exactly what its bridge writes — Factorio/Balatro JSON
//! lines, and the raw log lines of Isaac, TF2 and DST (as the file source
//! wraps them: `{"line": "…"}`).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use mlua::LuaSerdeExt;

type Calls = Arc<Mutex<Vec<(String, f64)>>>;

fn mods_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mods")
}

/// Loads a mod with a recording `vibe` stub; returns the Lua state and the
/// recorded (function, level) calls.
fn load_mod(source: &str) -> (mlua::Lua, Calls) {
    let lua = mlua::Lua::new();
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let vibe = lua.create_table().unwrap();
    let c = calls.clone();
    vibe.set(
        "pulse",
        lua.create_function(move |_, (level, _s): (f64, f64)| {
            c.lock().unwrap().push(("pulse".into(), level));
            Ok(())
        })
        .unwrap(),
    )
    .unwrap();
    let c = calls.clone();
    vibe.set(
        "set",
        lua.create_function(move |_, level: f64| {
            c.lock().unwrap().push(("set".into(), level));
            Ok(())
        })
        .unwrap(),
    )
    .unwrap();
    vibe.set("log", lua.create_function(|_, _: String| Ok(())).unwrap()).unwrap();
    vibe.set("status", lua.create_function(|_, _: String| Ok(())).unwrap()).unwrap();
    vibe.set("now", lua.create_function(|_, ()| Ok(1.0f64)).unwrap()).unwrap();
    lua.globals().set("vibe", vibe).unwrap();
    lua.load(source).exec().unwrap();
    (lua, calls)
}

fn send(lua: &mlua::Lua, json: &str) {
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    let on_message: mlua::Function = lua.globals().get("on_message").unwrap();
    on_message.call::<()>(("log", lua.to_value(&value).unwrap())).unwrap();
}

fn send_line(lua: &mlua::Lua, line: &str) {
    send(lua, &serde_json::json!({ "line": line }).to_string());
}

fn pulses(calls: &Calls) -> Vec<f64> {
    calls.lock().unwrap().iter().filter(|(k, _)| k == "pulse").map(|(_, v)| *v).collect()
}

#[test]
fn factorio_reacts_to_bridge_events() {
    let src = std::fs::read_to_string(mods_dir().join("factorio.lua")).unwrap();
    let (lua, calls) = load_mod(&src);
    send(&lua, r#"{"e":"dmg","f":0.3}"#);
    send(&lua, r#"{"e":"died"}"#);
    send(&lua, r#"{"e":"research"}"#);
    let p = pulses(&calls);
    assert!(p.iter().any(|v| (0.55..0.7).contains(v)), "damage pulse missing: {p:?}");
    assert!(p.iter().any(|v| *v >= 0.9), "death pulse missing: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.5).abs() < 0.01), "research pulse missing: {p:?}");
}

#[test]
fn balatro_reacts_to_bridge_events() {
    let src = std::fs::read_to_string(mods_dir().join("balatro.lua")).unwrap();
    let (lua, calls) = load_mod(&src);
    send(&lua, r#"{"e":"score","chips":300,"target":600}"#);
    send(&lua, r#"{"e":"state","s":"ROUND_EVAL","boss":true}"#);
    send(&lua, r#"{"e":"state","s":"GAME_OVER","boss":false}"#);
    let p = pulses(&calls);
    assert!(p.iter().any(|v| (0.35..0.5).contains(v)), "half-progress score pulse: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.85).abs() < 0.01), "boss blind pulse: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.9).abs() < 0.01), "game over pulse: {p:?}");
}

#[test]
fn isaac_parses_log_markers() {
    let src = std::fs::read_to_string(mods_dir().join("binding_of_isaac.lua")).unwrap();
    let (lua, calls) = load_mod(&src);
    send_line(&lua, "[INFO] - Lua Debug: VIBELOOP dmg 2.0 6");
    send_line(&lua, "[INFO] - Lua Debug: VIBELOOP boss");
    send_line(&lua, "[INFO] - some unrelated engine noise");
    let p = pulses(&calls);
    assert!(p.iter().any(|v| (0.7..0.85).contains(v)), "1/3 hearts damage pulse: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.7).abs() < 0.01), "boss pulse: {p:?}");
    assert_eq!(p.len(), 2, "noise line must not pulse: {p:?}");
}

#[test]
fn tf2_attributes_kill_feed_lines() {
    let src = std::fs::read_to_string(mods_dir().join("team_fortress2.lua")).unwrap();
    // The shipped file requires the player to fill in MY_NICK; do the same.
    let src = src.replace(r#"local MY_NICK = """#, r#"local MY_NICK = "TestGuy""#);
    let (lua, calls) = load_mod(&src);
    send_line(&lua, "TestGuy killed Bot01 with scattergun.");
    send_line(&lua, "TestGuy killed Bot02 with scattergun. (crit)");
    send_line(&lua, "Bot03 killed TestGuy with sniperrifle.");
    send_line(&lua, "Bot04 killed Bot05 with flamethrower.");
    let p = pulses(&calls);
    assert!(p.iter().any(|v| (*v - 0.55).abs() < 0.01), "kill pulse: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.75).abs() < 0.01), "crit kill pulse: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.85).abs() < 0.01), "death pulse: {p:?}");
    assert_eq!(p.len(), 3, "other people's kills must not pulse: {p:?}");
}

#[test]
fn minecraft_reacts_to_bridge_events() {
    let src = std::fs::read_to_string(mods_dir().join("minecraft.lua")).unwrap();
    let (lua, calls) = load_mod(&src);
    send(&lua, r#"{"e":"dmg","f":0.25}"#); // 2.5 hearts on 20 HP
    send(&lua, r#"{"e":"died"}"#);
    send(&lua, r#"{"e":"levelup","n":30}"#);
    let p = pulses(&calls);
    assert!(p.iter().any(|v| (0.6..0.7).contains(v)), "damage pulse: {p:?}");
    assert!(p.iter().any(|v| *v >= 0.9), "death pulse: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.4).abs() < 0.01), "level-up pulse: {p:?}");
}

#[test]
fn repo_reacts_to_bridge_events() {
    let src = std::fs::read_to_string(mods_dir().join("repo.lua")).unwrap();
    let (lua, calls) = load_mod(&src);
    send(&lua, r#"{"e":"dmg","f":0.3}"#);
    send(&lua, r#"{"e":"died"}"#);
    send(&lua, r#"{"e":"heal"}"#);
    let p = pulses(&calls);
    assert!(p.iter().any(|v| (0.65..0.75).contains(v)), "damage pulse: {p:?}");
    assert!(p.iter().any(|v| *v >= 0.9), "death pulse: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.2).abs() < 0.01), "heal tickle: {p:?}");
}

#[test]
fn webfishing_reacts_to_bridge_events() {
    let src = std::fs::read_to_string(mods_dir().join("webfishing.lua")).unwrap();
    let (lua, calls) = load_mod(&src);
    send(&lua, r#"{"e":"bite"}"#);
    send(&lua, r#"{"e":"catch","n":1}"#);
    send(&lua, r#"{"e":"levelup","n":12}"#);
    let p = pulses(&calls);
    assert!(p.iter().any(|v| (*v - 0.8).abs() < 0.01), "bite pulse: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.55).abs() < 0.01), "catch pulse: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.45).abs() < 0.01), "levelup pulse: {p:?}");
}

#[test]
fn war_thunder_feels_tank_battles() {
    // Ground vehicles report valid=false on /state (it's flight telemetry).
    // The battle feed must still work — regression test for the bug where
    // every state poll reset feed priming and muted tank battles entirely.
    let src = std::fs::read_to_string(mods_dir().join("war_thunder.lua")).unwrap();
    let (lua, calls) = load_mod(&src);
    let send_to = |source: &str, json: &str| {
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        let on_message: mlua::Function = lua.globals().get("on_message").unwrap();
        on_message.call::<()>((source, lua.to_value(&value).unwrap())).unwrap();
    };
    send_to("state", r#"{"valid":false}"#);
    // First feed poll mid-battle: backlog, swallowed silently.
    send_to("feed", r#"{"damage":[{"id":1,"msg":"A destroyed B"}]}"#);
    send_to("state", r#"{"valid":false}"#);
    // A new destruction after priming — with valid still false (tank battle).
    send_to(
        "feed",
        r#"{"damage":[{"id":1,"msg":"A destroyed B"},{"id":2,"msg":"C destroyed D"}]}"#,
    );
    let p = pulses(&calls);
    assert_eq!(p.len(), 1, "exactly the new event must pulse: {p:?}");
    assert!((p[0] - 0.35).abs() < 0.01, "unfiltered destruction pulse: {p:?}");
}

#[test]
fn war_thunder_variants_know_whose_hit_it_was() {
    // The feed lists attacker BEFORE the verb, victim AFTER. With MY_NICK
    // set, punishing must slam incoming hits and shrug at your kills, and
    // rewarding must purr at your hits and ignore incoming ones entirely.
    let feed = concat!(
        r#"{"damage":["#,
        r#"{"id":1,"msg":"FullOfSense (Leopard 2A4) damaged Foe (T-72A)"},"#,
        r#"{"id":2,"msg":"Foe (T-72A) critically damaged FullOfSense (Leopard 2A4)"},"#,
        r#"{"id":3,"msg":"FullOfSense (Leopard 2A4) destroyed Foe (T-72A)"},"#,
        r#"{"id":4,"msg":"Foe2 (T-80B) destroyed FullOfSense (Leopard 2A4)"},"#,
        r#"{"id":5,"msg":"Other (M1) destroyed Bystander (Object 279)"}"#,
        r#"]}"#
    );
    for (file, expected) in [
        // (your hit, hit on you, your kill, your death; bystanders silent)
        ("war_thunder_punishing.lua", vec![0.2, 0.8, 0.35, 1.0]),
        ("war_thunder_rewarding.lua", vec![0.25, 0.5]),
    ] {
        let src = std::fs::read_to_string(mods_dir().join(file))
            .unwrap()
            .replace("local MY_NICK = \"\"", "local MY_NICK = \"FullOfSense\"");
        let (lua, calls) = load_mod(&src);
        let send_to = |source: &str, json: &str| {
            let value: serde_json::Value = serde_json::from_str(json).unwrap();
            let on_message: mlua::Function = lua.globals().get("on_message").unwrap();
            on_message.call::<()>((source, lua.to_value(&value).unwrap())).unwrap();
        };
        // Prime on an empty battle start, then deliver the five events.
        send_to("feed", r#"{"damage":[]}"#);
        send_to("feed", feed);
        let p = pulses(&calls);
        assert_eq!(p, expected, "{file}: wrong pulses");
    }
}

#[test]
fn dst_parses_bridge_markers() {
    let src = std::fs::read_to_string(mods_dir().join("dont_starve_together.lua")).unwrap();
    let (lua, calls) = load_mod(&src);
    send_line(&lua, "[00:01:23]: VIBELOOP dmg 0.250");
    send_line(&lua, "[00:02:00]: VIBELOOP died");
    send_line(&lua, "[00:02:05]: unrelated chatter");
    let p = pulses(&calls);
    assert!(p.iter().any(|v| (*v - 0.7).abs() < 0.01), "25% health damage pulse: {p:?}");
    assert!(p.iter().any(|v| *v >= 0.9), "death pulse: {p:?}");
}
