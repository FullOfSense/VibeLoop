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
    // Per-action events: play commit, three scoring pops, a 4-card
    // discard, a draw and a tarot use.
    send(&lua, r#"{"e":"play","n":2}"#);
    for _ in 0..3 {
        send(&lua, r#"{"e":"pop","chips":45,"mult":4}"#);
    }
    send(&lua, r#"{"e":"discard","n":4}"#);
    send(&lua, r#"{"e":"draw"}"#);
    send(&lua, r#"{"e":"use"}"#);
    // The count-up: play resets the combo, then each cardpop escalates.
    // chips amt=30 → 0.15+0.12 base, +0.02 combo = 0.29; x_mult → 0.30
    // base +0.04 = 0.34; mult → 0.22 base +0.06 = 0.28.
    send(&lua, r#"{"e":"play","n":5}"#);
    send(&lua, r#"{"e":"cardpop","t":"chips","amt":30}"#);
    send(&lua, r#"{"e":"cardpop","t":"x_mult","amt":3}"#);
    send(&lua, r#"{"e":"cardpop","t":"mult","amt":4}"#);
    let p = pulses(&calls);
    assert!(p.iter().any(|v| (*v - 0.29).abs() < 0.001), "chip cardpop scales with amt: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.34).abs() < 0.001), "x_mult cardpop escalated: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.28).abs() < 0.001), "mult cardpop escalated: {p:?}");
    assert!(p.iter().any(|v| (0.35..0.5).contains(v)), "half-progress score pulse: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.85).abs() < 0.01), "boss blind pulse: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.9).abs() < 0.01), "game over pulse: {p:?}");
    assert_eq!(
        p.iter().filter(|v| (**v - 0.18).abs() < 0.001).count(),
        3,
        "three scoring pops must each tick: {p:?}"
    );
    assert!(p.iter().any(|v| (*v - 0.32).abs() < 0.001), "4-card discard pulse: {p:?}");
    assert!(p.iter().any(|v| (*v - 0.12).abs() < 0.001), "draw tap: {p:?}");
    assert_eq!(
        p.iter().filter(|v| (**v - 0.3).abs() < 0.001).count(),
        3,
        "two play commits and the tarot use each pulse 0.3: {p:?}"
    );
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
    // Indicator sequence: baseline → cannon shot (ammo 6→5) → crew loss
    // (4→3) → wall ram (speed 33→1, impact 32) → rack reload (5→6, silent).
    let ind = [
        r#"{"valid":true,"army":"tank","first_stage_ammo":6,"crew_current":4,"speed":33}"#,
        r#"{"valid":true,"army":"tank","first_stage_ammo":5,"crew_current":4,"speed":33}"#,
        r#"{"valid":true,"army":"tank","first_stage_ammo":5,"crew_current":3,"speed":33}"#,
        r#"{"valid":true,"army":"tank","first_stage_ammo":5,"crew_current":3,"speed":1}"#,
        r#"{"valid":true,"army":"tank","first_stage_ammo":6,"crew_current":3,"speed":1}"#,
    ];
    for (file, expected) in [
        // Punishing: your hit/kill silent; crit on you 0.8, death 1.0;
        // crew loss 0.85; ram 0.4 + 32/60*0.4.
        ("war_thunder_punishing.lua", vec![0.8, 1.0, 0.85, 0.4 + 32.0 / 60.0 * 0.4]),
        // Rewarding: your hit 0.25, damage tickle 0.2, kill 0.5, death
        // tickle 0.2; shot 0.35, crew loss 0.2, ram 0.2 + 32/60*0.3.
        ("war_thunder_rewarding.lua", vec![0.25, 0.2, 0.5, 0.2, 0.35, 0.2, 0.2 + 32.0 / 60.0 * 0.3]),
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
        // Prime on an empty battle start, then deliver the five feed
        // events, then the indicator sequence.
        send_to("feed", r#"{"damage":[]}"#);
        send_to("feed", feed);
        for payload in &ind {
            send_to("ind", payload);
        }
        let p = pulses(&calls);
        assert_eq!(p.len(), expected.len(), "{file}: pulse count, got {p:?}");
        for (got, want) in p.iter().zip(&expected) {
            assert!((got - want).abs() < 0.001, "{file}: expected {expected:?}, got {p:?}");
        }
    }
}

#[test]
fn cs2_variants_feel_the_right_things() {
    // A GSI payload builder: hp, ak clip, round phase (+ winner).
    let payload = |hp: i64, clip: i64, phase: &str, winner: &str| {
        let round = if phase == "over" {
            format!(r#"{{"phase":"over","win_team":"{winner}"}}"#)
        } else {
            format!(r#"{{"phase":"{phase}"}}"#)
        };
        format!(
            r#"{{"provider":{{"steamid":"765"}},"round":{round},
              "player":{{"steamid":"765","team":"CT",
                "state":{{"health":{hp},"round_kills":0,"flashed":0,"burning":0}},
                "weapons":{{"weapon_1":{{"name":"weapon_ak47","state":"active","ammo_clip":{clip}}}}}}}}}"#
        )
    };
    for (file, expected) in [
        // Rewarding: 3-round burst 0.15+0.12, 25 damage tap, soft death
        // 0.3, round-loss nod 0.25 (fires once despite two "over" posts).
        ("counterstrike2_rewarding.lua", vec![0.27, 0.2125, 0.3, 0.25]),
        // Punishing: shots and damage silent; death 1.0, round loss 0.8.
        ("counterstrike2_punishing.lua", vec![1.0, 0.8]),
    ] {
        let src = std::fs::read_to_string(mods_dir().join(file)).unwrap();
        let (lua, calls) = load_mod(&src);
        let send_to = |json: &str| {
            let value: serde_json::Value = serde_json::from_str(json).unwrap();
            let on_message: mlua::Function = lua.globals().get("on_message").unwrap();
            on_message.call::<()>(("gsi", lua.to_value(&value).unwrap())).unwrap();
        };
        send_to(&payload(100, 30, "live", ""));  // baseline
        send_to(&payload(100, 27, "live", ""));  // 3-round burst
        send_to(&payload(75, 27, "live", ""));   // took 25 damage
        send_to(&payload(0, 27, "live", ""));    // died
        send_to(&payload(0, 27, "over", "T"));   // round lost...
        send_to(&payload(0, 27, "over", "T"));   // ...posted twice
        let p = pulses(&calls);
        assert_eq!(p.len(), expected.len(), "{file}: pulse count, got {p:?}");
        for (got, want) in p.iter().zip(&expected) {
            assert!((got - want).abs() < 0.001, "{file}: expected {expected:?}, got {p:?}");
        }
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
