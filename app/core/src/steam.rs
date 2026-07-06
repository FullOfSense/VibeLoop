//! Steam library discovery: every `steamapps` folder on the machine, across
//! all drives, read from Valve's own `libraryfolders.vdf`. Used by the file
//! source (`${STEAM_LIBRARIES}` path candidates find Proton prefixes and game
//! folders wherever the user installed them) and by the app's setup doctor.

use std::collections::HashSet;
use std::path::PathBuf;

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
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
        if let Some(p) = std::env::var_os(var).map(PathBuf::from) {
            roots.push(p.join("Steam"));
        }
    }
    roots.push(PathBuf::from("C:\\Program Files (x86)\\Steam"));
    roots
}

/// Every steamapps folder on the machine: the default one(s) plus everything
/// listed in libraryfolders.vdf (extra drives).
pub fn steamapps_dirs() -> Vec<PathBuf> {
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
pub fn vdf_values(text: &str, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('"').collect();
        if parts.len() >= 4 && parts[1] == key && !parts[3].is_empty() {
            out.push(parts[3].replace("\\\\", "\\"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vdf_extracts_paths_and_unescapes() {
        let text = "\"0\"\n{\n\t\"path\"\t\t\"/mnt/HDD/SteamLibrary\"\n}\n\"1\"\n{\n\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t\"label\"\t\t\"\"\n}";
        assert_eq!(vdf_values(text, "path"), vec!["/mnt/HDD/SteamLibrary", "D:\\SteamLibrary"]);
        assert!(vdf_values(text, "label").is_empty()); // empty values skipped
    }
}
