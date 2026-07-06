//! Dev tool: print the setup-doctor report for every shipped mod against
//! THIS machine's disk. Run with:
//!     cargo run -p vibeloop --example setup_doctor --no-default-features

use std::path::PathBuf;

fn main() {
    let mods = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mods");
    let ctx = vibeloop_lib::setup::Ctx::new(vec![mods.clone()]);

    let mut ids: Vec<String> = std::fs::read_dir(&mods)
        .unwrap()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            (p.extension()?.to_str()? == "lua")
                .then(|| p.file_stem().unwrap().to_string_lossy().into_owned())
        })
        .collect();
    ids.sort();

    for id in ids {
        let r = vibeloop_lib::setup::report(&ctx, &id);
        println!("── {id} {}", if r.ready { "— READY" } else { "" });
        for s in r.steps {
            let mark = match s.state.as_str() {
                "ok" => "✔",
                "todo" => "↓",
                "manual" => "✋",
                _ => "?",
            };
            println!("   {mark} [{}] {}", s.state, s.title);
            if !s.detail.is_empty() {
                println!("       {}", s.detail);
            }
            if let Some(u) = s.url {
                println!("       ↗ {u}");
            }
        }
    }
}
