//! Per-mod setup doctor. For every shipped mod it knows the dependency chain
//! (game install → framework → our companion bridge), checks each piece on
//! disk, and one-click-installs the pieces we ship ourselves. Everything we
//! can't do (install BepInEx, enable OSC, set a launch option) becomes a step
//! with a guide link instead of a silent "read the README".
//!
//! Pure std + serde — no tauri types, so it's unit-testable on its own.

use serde::Serialize;
use std::collections::HashSet;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

// ─── Public shape (what the frontend renders) ────────────────────────────────

#[derive(Serialize, Clone)]
pub struct StepStatus {
    pub id: String,
    pub title: String,
    pub detail: String,
    /// "ok"      → done / detected
    /// "todo"    → we can do it: `action == "install"`
    /// "manual"  → the user must act (guide `url` and/or mods `folder` action)
    /// "unknown" → can't verify (e.g. game not found)
    pub state: String,
    /// "install" → call `mod_setup_apply` with this step id
    /// "folder"  → open the mods folder (user edits a .lua there)
    pub action: Option<String>,
    pub url: Option<String>,
}

#[derive(Serialize)]
pub struct SetupReport {
    pub mod_id: String,
    /// false → mod we don't know (user-made): frontend falls back to @setup.
    pub supported: bool,
    pub ready: bool,
    pub steps: Vec<StepStatus>,
}

// ─── Internal step (status + optional install plan) ─────────────────────────

struct Step {
    status: StepStatus,
    /// (from, to): copied recursively when the user clicks Install.
    plan: Option<(PathBuf, PathBuf)>,
}

fn step(id: &str, state: &str, title: impl Into<String>, detail: impl Into<String>) -> Step {
    Step {
        status: StepStatus {
            id: id.into(),
            title: title.into(),
            detail: detail.into(),
            state: state.into(),
            action: None,
            url: None,
        },
        plan: None,
    }
}

impl Step {
    fn url(mut self, url: &str) -> Self {
        self.status.url = Some(url.into());
        self
    }
    fn folder(mut self) -> Self {
        self.status.action = Some("folder".into());
        self
    }
    fn install(mut self, from: PathBuf, to: PathBuf) -> Self {
        self.status.action = Some("install".into());
        self.plan = Some((from, to));
        self
    }
}

// ─── Context: where things live on this machine ─────────────────────────────

pub struct Ctx {
    /// Mod directories in engine priority order (first hit wins), used to
    /// find companion files and the shipped .lua files.
    mods_dirs: Vec<PathBuf>,
    steamapps: Vec<PathBuf>,
}

impl Ctx {
    pub fn new(mods_dirs: Vec<PathBuf>) -> Self {
        Self {
            mods_dirs,
            steamapps: discover_steamapps(),
        }
    }

    /// First existing `companions/<rel>` across the mod dirs.
    fn companion(&self, rel: &str) -> Option<PathBuf> {
        self.mods_dirs
            .iter()
            .map(|d| d.join("companions").join(rel))
            .find(|p| p.exists())
    }

    /// First existing shipped mod file (same priority the engine uses).
    fn mod_file(&self, name: &str) -> Option<PathBuf> {
        self.mods_dirs
            .iter()
            .map(|d| d.join(name))
            .find(|p| p.is_file())
    }

    /// Steam game install dir by appid; falls back to the well-known folder
    /// name for libraries with a missing/unreadable manifest.
    fn steam_game(&self, appid: u32, folder: &str) -> Option<GameDir> {
        for sa in &self.steamapps {
            let manifest = sa.join(format!("appmanifest_{appid}.acf"));
            if let Ok(text) = std::fs::read_to_string(&manifest) {
                if let Some(installdir) = vdf_values(&text, "installdir").into_iter().next() {
                    let dir = sa.join("common").join(installdir);
                    if dir.is_dir() {
                        return Some(GameDir { dir, steamapps: sa.clone(), appid });
                    }
                }
            }
        }
        for sa in &self.steamapps {
            let dir = sa.join("common").join(folder);
            if dir.is_dir() {
                return Some(GameDir { dir, steamapps: sa.clone(), appid });
            }
        }
        None
    }
}

struct GameDir {
    dir: PathBuf,
    steamapps: PathBuf,
    appid: u32,
}

impl GameDir {
    /// The Windows-ish roaming AppData for a Proton game, if it exists.
    fn proton_roaming(&self) -> Option<PathBuf> {
        let p = self
            .steamapps
            .join("compatdata")
            .join(self.appid.to_string())
            .join("pfx/drive_c/users/steamuser/AppData/Roaming");
        p.is_dir().then_some(p)
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_dir())
}

fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(h) = home() {
        // Linux (native, symlink, flatpak), then macOS.
        roots.push(h.join(".local/share/Steam"));
        roots.push(h.join(".steam/steam"));
        roots.push(h.join(".steam/root"));
        roots.push(h.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
        roots.push(h.join("Library/Application Support/Steam"));
    }
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(p) = env_dir(var) {
            roots.push(p.join("Steam"));
        }
    }
    roots.push(PathBuf::from("C:\\Program Files (x86)\\Steam"));
    roots
}

/// Every steamapps folder on the machine: the default one(s) plus everything
/// listed in libraryfolders.vdf (extra drives).
fn discover_steamapps() -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut push = |p: PathBuf| {
        if p.is_dir() {
            let key = p.canonicalize().unwrap_or_else(|_| p.clone());
            if seen.insert(key) {
                out.push(p);
            }
        }
    };
    for root in steam_roots() {
        let sa = root.join("steamapps");
        if let Ok(text) = std::fs::read_to_string(sa.join("libraryfolders.vdf")) {
            for lib in vdf_values(&text, "path") {
                push(PathBuf::from(lib).join("steamapps"));
            }
        }
        push(sa);
    }
    out
}

/// All values for `"key" "value"` lines in Valve's VDF/ACF format.
/// Windows paths come escaped (`\\`) — unescaped here.
fn vdf_values(text: &str, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('"').collect();
        if parts.len() >= 4 && parts[1] == key && !parts[3].is_empty() {
            out.push(parts[3].replace("\\\\", "\\"));
        }
    }
    out
}

fn port_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(250),
    )
    .is_ok()
}

/// The `MY_NICK = "..."` value in a mod file, if the file exists.
fn my_nick(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .find(|l| l.contains("MY_NICK") && l.contains('='))
        .and_then(|l| l.split('"').nth(1).map(str::to_string))
}

fn dir_contains(dir: &Path, pred: impl Fn(&str) -> bool) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .any(|e| pred(&e.file_name().to_string_lossy().to_lowercase()))
        })
        .unwrap_or(false)
}

fn copy_recursive(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)?.flatten() {
            copy_recursive(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(from, to)?;
    }
    Ok(())
}

// ─── Shared step builders ────────────────────────────────────────────────────

/// "Copy our companion <rel> to <target>" with detection + graceful fallbacks.
fn bridge_step(
    ctx: &Ctx,
    id: &str,
    title: &str,
    rel: &str,
    target: Option<PathBuf>,
    installed: bool,
    missing_target_detail: &str,
    after_install_note: &str,
) -> Step {
    if installed {
        return step(id, "ok", title, format!("Installed. {after_install_note}").trim().to_string());
    }
    let Some(target) = target else {
        return step(id, "unknown", title, missing_target_detail).folder();
    };
    match ctx.companion(rel) {
        Some(src) => step(
            id,
            "todo",
            title,
            format!("One click — VibeLoop copies it for you. {after_install_note}")
                .trim()
                .to_string(),
        )
        .install(src, target),
        None => step(
            id,
            "unknown",
            title,
            format!("Companion files missing from your mods folder (companions/{rel}). Reinstalling VibeLoop restores them."),
        )
        .folder(),
    }
}

/// The optional/required MY_NICK edit shared by War Thunder and TF2.
fn nick_step(ctx: &Ctx, file: &str, required: bool, why: &str) -> Step {
    match ctx.mod_file(file).and_then(|p| my_nick(&p)) {
        Some(nick) if !nick.is_empty() => step(
            "nick",
            "ok",
            format!("Your nickname is set: “{nick}”"),
            "",
        ),
        _ => step(
            "nick",
            if required { "manual" } else { "unknown" },
            if required { "Set your in-game nickname" } else { "Set your nickname (optional)" },
            format!("{why} Open the mods folder, edit MY_NICK at the top of {file}, then Re-check."),
        )
        .folder(),
    }
}

// ─── Per-game checkers ───────────────────────────────────────────────────────

fn steps_for(ctx: &Ctx, mod_id: &str) -> Option<Vec<Step>> {
    Some(match mod_id {
        "demo_test" => vec![step(
            "none",
            "ok",
            "Nothing to set up",
            "The demo runs on its own — press START to feel the test pattern.",
        )],

        "osu_rewarding" | "osu_punishing" => vec![if port_open(24050) {
            step("tosu", "ok", "tosu is running", "osu! data is flowing — press START and play.")
        } else {
            step(
                "tosu",
                "manual",
                "Run tosu",
                "Download tosu, unzip it anywhere and run it alongside osu! (stable or lazer). \
                 Keep it running while you play, then Re-check.",
            )
            .url("https://github.com/tosuapp/tosu/releases")
        }],

        "league_of_legends" => vec![step(
            "api",
            "ok",
            "Nothing to install",
            if port_open(2999) {
                "League's built-in Live Client API is on — you're in a match right now, press START."
            } else {
                "League's Live Client API is built into the game. Just be in a match (not the lobby) and it works."
            },
        )],

        "war_thunder" => vec![
            step(
                "api",
                "ok",
                "Nothing to install",
                if port_open(8111) {
                    "War Thunder's built-in localhost API is live right now."
                } else {
                    "War Thunder's localhost API is built in and always on — just start the game."
                },
            ),
            nick_step(
                ctx,
                "war_thunder.lua",
                false,
                "Without it you feel the whole battle feed, not just your own hits and kills.",
            ),
        ],

        "counterstrike2" => {
            let game = ctx.steam_game(730, "Counter-Strike Global Offensive");
            vec![match &game {
                Some(g) => {
                    let target = g.dir.join("game/csgo/cfg/gamestate_integration_vibeloop.cfg");
                    bridge_step(
                        ctx,
                        "gsi",
                        "Game State Integration config in CS2",
                        "cs2/gamestate_integration_vibeloop.cfg",
                        Some(target.clone()),
                        target.is_file(),
                        "",
                        "Restart CS2 if it was running.",
                    )
                }
                None => step(
                    "gsi",
                    "unknown",
                    "Game State Integration config in CS2",
                    "Couldn't find CS2 in your Steam libraries. Copy the cfg from mods/companions/cs2/ \
                     into …/Counter-Strike Global Offensive/game/csgo/cfg/ yourself.",
                )
                .folder(),
            }]
        }

        "vrchat" => vec![
            step(
                "osc",
                "manual",
                "Enable OSC in VRChat",
                "In-game: open the Action Menu → Options → OSC → Enabled. VRChat remembers it.",
            )
            .url("https://docs.vrchat.com/docs/osc-overview"),
            step(
                "param",
                "manual",
                "Avatar parameter “VibeLoop”",
                "Your avatar needs a float parameter named VibeLoop (usually driven by a Contact \
                 Receiver). Any avatar that has it works instantly.",
            ),
        ],

        "beat_saber" => {
            let game = ctx.steam_game(620980, "Beat Saber");
            vec![match &game {
                Some(g)
                    if g.dir.join("Plugins/DataPuller.dll").is_file()
                        || g.dir.join("IPA/Pending/Plugins/DataPuller.dll").is_file() =>
                {
                    step("datapuller", "ok", "DataPuller mod installed", "Press START, then play a map.")
                }
                Some(_) => step(
                    "datapuller",
                    "manual",
                    "Install the DataPuller mod",
                    "Use ModAssistant or BSManager and install “DataPuller”, then Re-check. \
                     First time modding Beat Saber? The guide covers it.",
                )
                .url("https://bsmg.wiki/pc-modding.html"),
                None => step(
                    "datapuller",
                    "unknown",
                    "Install the DataPuller mod",
                    "Beat Saber wasn't found in your Steam libraries — install the game first, \
                     then mod it with ModAssistant/BSManager (mod: DataPuller).",
                )
                .url("https://bsmg.wiki/pc-modding.html"),
            }]
        }

        "factorio" => {
            let data = [
                home().map(|h| h.join(".factorio")),
                env_dir("APPDATA").map(|a| a.join("Factorio")),
                home().map(|h| h.join("Library/Application Support/factorio")),
            ]
            .into_iter()
            .flatten()
            .find(|p| p.is_dir());
            let target = data.as_ref().map(|d| d.join("mods/vibeloop-bridge"));
            let installed = target.as_ref().map(|t| t.is_dir()).unwrap_or(false);
            vec![bridge_step(
                ctx,
                "bridge",
                "VibeLoop bridge in your Factorio mods",
                "factorio/vibeloop-bridge",
                target,
                installed,
                "Factorio's user folder wasn't found — run Factorio once, then Re-check.",
                "Factorio picks it up on the next launch (Mods menu shows “VibeLoop Bridge”).",
            )]
        }

        "balatro" => {
            let game = ctx.steam_game(2379780, "Balatro");
            let data = [
                env_dir("APPDATA").map(|a| a.join("Balatro")),
                home().map(|h| h.join("Library/Application Support/Balatro")),
                game.as_ref().and_then(|g| g.proton_roaming()).map(|r| r.join("Balatro")),
            ]
            .into_iter()
            .flatten()
            .find(|p| p.is_dir());

            let mods = data.as_ref().map(|d| d.join("Mods"));
            let smods = mods
                .as_ref()
                .map(|m| dir_contains(m, |n| n.contains("smods") || n.contains("steamodded")))
                .unwrap_or(false);
            let installed = mods.as_ref().map(|m| m.join("VibeLoopBridge").is_dir()).unwrap_or(false);

            vec![
                if smods {
                    step("smods", "ok", "Steamodded installed", "")
                } else {
                    step(
                        "smods",
                        "manual",
                        "Install Steamodded (+ lovely)",
                        "Balatro mods need the lovely injector and Steamodded. The wiki's \
                         install guide covers both in ~5 minutes; then Re-check.",
                    )
                    .url("https://github.com/Steamodded/smods/wiki")
                },
                bridge_step(
                    ctx,
                    "bridge",
                    "VibeLoop bridge in Balatro/Mods",
                    "balatro/VibeLoopBridge",
                    mods.map(|m| m.join("VibeLoopBridge")),
                    installed,
                    "Balatro's save folder wasn't found — run Balatro once, then Re-check.",
                    "",
                ),
            ]
        }

        "binding_of_isaac" => {
            let game = ctx.steam_game(250900, "The Binding of Isaac Rebirth");
            let target = game.as_ref().map(|g| g.dir.join("mods/vibeloop bridge"));
            let installed = target
                .as_ref()
                .map(|t| t.join("main.lua").is_file())
                .unwrap_or(false);
            vec![bridge_step(
                ctx,
                "bridge",
                "VibeLoop bridge in Isaac's mods",
                "isaac/vibeloop bridge",
                target,
                installed,
                "Isaac wasn't found in your Steam libraries. Copy companions/isaac/ into the \
                 game's mods/ folder yourself.",
                "Needs Repentance or Repentance+. Enable it in-game: Mods → “vibeloop bridge”.",
            )]
        }

        "team_fortress2" => {
            let game = ctx.steam_game(440, "Team Fortress 2");
            let condebug = game
                .as_ref()
                .map(|g| g.dir.join("tf/console.log").is_file())
                .unwrap_or(false);
            vec![
                if condebug {
                    step(
                        "condebug",
                        "ok",
                        "Console logging is on",
                        "console.log found — TF2 has run with -condebug.",
                    )
                } else {
                    step(
                        "condebug",
                        "manual",
                        "Add TF2 launch options",
                        "Steam → right-click Team Fortress 2 → Properties → Launch Options: \
                         -condebug -conclearlog   — then launch TF2 once and Re-check.",
                    )
                    .url("https://help.steampowered.com/en/faqs/view/7D01-D2DD-D75E-2955")
                },
                nick_step(
                    ctx,
                    "team_fortress2.lua",
                    true,
                    "The kill feed only makes sense if the mod knows which name is you.",
                ),
            ]
        }

        "dont_starve_together" => {
            let game = ctx.steam_game(322330, "Don't Starve Together");
            let target = game.as_ref().map(|g| g.dir.join("mods/vibeloop-bridge"));
            let installed = target
                .as_ref()
                .map(|t| t.join("modmain.lua").is_file())
                .unwrap_or(false);
            vec![bridge_step(
                ctx,
                "bridge",
                "VibeLoop bridge in DST's mods",
                "dst/vibeloop-bridge",
                target,
                installed,
                "Don't Starve Together wasn't found in your Steam libraries. Copy \
                 companions/dst/vibeloop-bridge into the game's mods/ folder yourself.",
                "Enable it in-game: Mods → Client Mods → “VibeLoop Bridge” (works on any server).",
            )]
        }

        "minecraft" => {
            let mc = [
                env_dir("APPDATA").map(|a| a.join(".minecraft")),
                home().map(|h| h.join(".minecraft")),
                home().map(|h| h.join("Library/Application Support/minecraft")),
            ]
            .into_iter()
            .flatten()
            .find(|p| p.is_dir());

            let Some(mc) = mc else {
                return Some(vec![step(
                    "mc",
                    "unknown",
                    "Minecraft folder not found",
                    "No .minecraft folder on this machine. Using another launcher (Prism, \
                     MultiMC…)? Add Fabric 1.21.1 + Fabric API there and drop our jar from \
                     companions/minecraft/ into its mods folder.",
                )
                .folder()]);
            };

            let fabric = mc.join("versions").is_dir()
                && dir_contains(&mc.join("versions"), |n| n.starts_with("fabric-loader"));
            let fabric_api = dir_contains(&mc.join("mods"), |n| n.contains("fabric-api") && n.ends_with(".jar"));
            let bridge = dir_contains(&mc.join("mods"), |n| n.starts_with("vibeloop-bridge") && n.ends_with(".jar"));

            vec![
                if fabric {
                    step("fabric", "ok", "Fabric Loader installed", "")
                } else {
                    step(
                        "fabric",
                        "manual",
                        "Install Fabric Loader",
                        "Run the Fabric installer and pick Minecraft 1.21.1 (the version our \
                         bridge is built for), then Re-check.",
                    )
                    .url("https://fabricmc.net/use/installer/")
                },
                if fabric_api {
                    step("fabric-api", "ok", "Fabric API installed", "")
                } else {
                    step(
                        "fabric-api",
                        "manual",
                        "Install Fabric API",
                        "Download the Fabric API jar for 1.21.1 and drop it into .minecraft/mods, \
                         then Re-check.",
                    )
                    .url("https://modrinth.com/mod/fabric-api")
                },
                bridge_step(
                    ctx,
                    "bridge",
                    "VibeLoop bridge jar in .minecraft/mods",
                    "minecraft/vibeloop-bridge-1.0.0.jar",
                    Some(mc.join("mods/vibeloop-bridge-1.0.0.jar")),
                    bridge,
                    "",
                    "Launch the “fabric-loader-1.21.1” profile. Works with any server.",
                ),
            ]
        }

        "repo" => {
            let game = ctx.steam_game(3241660, "REPO");
            let Some(g) = game else {
                return Some(vec![step(
                    "game",
                    "unknown",
                    "R.E.P.O. not found",
                    "The game wasn't found in your Steam libraries — install it first, then Re-check.",
                )]);
            };
            let bepinex = g.dir.join("BepInEx/core/BepInEx.dll").is_file();
            let plugin = g.dir.join("BepInEx/plugins/VibeLoopBridge.REPO.dll").is_file();

            let mut bepinex_detail = String::from(
                "Download BepInEx 5 (x64, the .zip) and unzip it straight into the game folder, \
                 so BepInEx/ sits next to REPO.exe. Launch the game once, then Re-check.",
            );
            if cfg!(target_os = "linux") {
                bepinex_detail.push_str(
                    " On Linux/Proton also set the game's launch options to: \
                     WINEDLLOVERRIDES=\"winhttp=n,b\" %command%",
                );
            }

            vec![
                if bepinex {
                    step("bepinex", "ok", "BepInEx 5 installed", "")
                } else {
                    step("bepinex", "manual", "Install BepInEx 5", bepinex_detail)
                        .url("https://github.com/BepInEx/BepInEx/releases")
                },
                if !bepinex && !plugin {
                    step(
                        "bridge",
                        "manual",
                        "VibeLoop plugin in BepInEx/plugins",
                        "Install BepInEx first — then this becomes one click.",
                    )
                } else {
                    bridge_step(
                        ctx,
                        "bridge",
                        "VibeLoop plugin in BepInEx/plugins",
                        "repo/VibeLoopBridge.REPO.dll",
                        Some(g.dir.join("BepInEx/plugins/VibeLoopBridge.REPO.dll")),
                        plugin,
                        "",
                        "",
                    )
                },
            ]
        }

        "webfishing" => {
            let game = ctx.steam_game(3146520, "WEBFISHING");
            let Some(g) = game else {
                return Some(vec![step(
                    "game",
                    "unknown",
                    "WEBFISHING not found",
                    "The game wasn't found in your Steam libraries — install it first, then Re-check.",
                )]);
            };
            let gdweave = g.dir.join("GDWeave").is_dir() || g.dir.join("winmm.dll").is_file();
            let installed = g.dir.join("GDWeave/Mods/FullOfSense.VibeLoop/manifest.json").is_file();

            vec![
                if gdweave {
                    step("gdweave", "ok", "GDWeave installed", "")
                } else {
                    step(
                        "gdweave",
                        "manual",
                        "Install GDWeave",
                        "Install GDWeave from Thunderstore (or the Hook, Line & Sinker mod \
                         manager), then Re-check.",
                    )
                    .url("https://thunderstore.io/c/webfishing/p/NotNet/GDWeave/")
                },
                if !gdweave && !installed {
                    step(
                        "bridge",
                        "manual",
                        "VibeLoop mod in GDWeave/Mods",
                        "Install GDWeave first — then this becomes one click.",
                    )
                } else {
                    bridge_step(
                        ctx,
                        "bridge",
                        "VibeLoop mod in GDWeave/Mods",
                        "webfishing/FullOfSense.VibeLoop",
                        Some(g.dir.join("GDWeave/Mods/FullOfSense.VibeLoop")),
                        installed,
                        "",
                        "",
                    )
                },
            ]
        }

        _ => return None,
    })
}

// ─── Entry points (called from tauri commands) ───────────────────────────────

pub fn report(ctx: &Ctx, mod_id: &str) -> SetupReport {
    match steps_for(ctx, mod_id) {
        Some(steps) => SetupReport {
            mod_id: mod_id.into(),
            supported: true,
            ready: steps.iter().all(|s| s.status.state == "ok"),
            steps: steps.into_iter().map(|s| s.status).collect(),
        },
        None => SetupReport {
            mod_id: mod_id.into(),
            supported: false,
            ready: false,
            steps: Vec::new(),
        },
    }
}

pub fn apply(ctx: &Ctx, mod_id: &str, step_id: &str) -> Result<SetupReport, String> {
    let steps = steps_for(ctx, mod_id)
        .ok_or_else(|| format!("No setup checker for mod '{mod_id}'."))?;
    let step = steps
        .into_iter()
        .find(|s| s.status.id == step_id)
        .ok_or_else(|| format!("Unknown setup step '{step_id}'."))?;
    let (from, to) = step
        .plan
        .ok_or_else(|| "This step has nothing VibeLoop can install — follow its guide instead.".to_string())?;
    copy_recursive(&from, &to)
        .map_err(|e| format!("Couldn't copy to {}: {e}", to.display()))?;
    Ok(report(ctx, mod_id))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vdf_extracts_paths_and_unescapes() {
        let text = r#"
"libraryfolders"
{
    "0"
    {
        "path"      "/home/u/.local/share/Steam"
    }
    "1"
    {
        "path"      "D:\\SteamLibrary"
        "label"     ""
    }
}
"#;
        assert_eq!(
            vdf_values(text, "path"),
            vec!["/home/u/.local/share/Steam", "D:\\SteamLibrary"]
        );
        assert!(vdf_values(text, "label").is_empty()); // empty values skipped
    }

    #[test]
    fn acf_extracts_installdir() {
        let text = "\"AppState\"\n{\n\t\"appid\"\t\t\"440\"\n\t\"installdir\"\t\t\"Team Fortress 2\"\n}";
        assert_eq!(vdf_values(text, "installdir"), vec!["Team Fortress 2"]);
    }

    #[test]
    fn my_nick_parses_set_and_empty() {
        let dir = std::env::temp_dir().join("vibeloop-setup-test");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("nick.lua");
        std::fs::write(&f, "-- header\nlocal MY_NICK = \"FullOfSense\"\n").unwrap();
        assert_eq!(my_nick(&f).as_deref(), Some("FullOfSense"));
        std::fs::write(&f, "local MY_NICK = \"\"\n").unwrap();
        assert_eq!(my_nick(&f).as_deref(), Some(""));
    }

    #[test]
    fn copy_recursive_copies_trees() {
        let base = std::env::temp_dir().join(format!("vibeloop-copy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let src = base.join("src/sub");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.txt"), "hello").unwrap();
        copy_recursive(&base.join("src"), &base.join("dst")).unwrap();
        assert_eq!(std::fs::read_to_string(base.join("dst/sub/a.txt")).unwrap(), "hello");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Every shipped mod must have a setup checker — this fails the moment a
    /// new mod lands without one.
    #[test]
    fn every_shipped_mod_is_supported() {
        let mods = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mods");
        let ctx = Ctx::new(vec![mods.clone()]);
        for entry in std::fs::read_dir(&mods).unwrap().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("lua") {
                continue;
            }
            let id = path.file_stem().unwrap().to_str().unwrap();
            assert!(
                steps_for(&ctx, id).is_some(),
                "mod '{id}' has no setup checker in setup.rs"
            );
        }
    }

    #[test]
    fn unknown_mod_reports_unsupported() {
        let ctx = Ctx::new(vec![]);
        let r = report(&ctx, "my_custom_mod");
        assert!(!r.supported);
        assert!(apply(&ctx, "my_custom_mod", "x").is_err());
    }
}
