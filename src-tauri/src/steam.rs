//! Installed Steam games, read from Steam's own files. Catches installs made after the import.

use crate::vdf;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

const CDN: &str = "https://cdn.cloudflare.steamstatic.com/steam/apps";

#[derive(Debug, Clone, PartialEq)]
pub struct SteamApp {
    pub app_id: u32,
    pub name: String,
    pub install_dir: PathBuf,
    pub size_bytes: u64,
    pub cover: String,
    pub hero: String,
    pub logo: String,
}

pub struct Steam {
    pub root: PathBuf,
    pub apps: HashMap<u32, SteamApp>,
}

impl Steam {
    pub fn art_dir(&self) -> PathBuf {
        cache_in(&self.root)
    }
}

fn cache_in(root: &Path) -> PathBuf {
    root.join("appcache").join("librarycache")
}

/// Steam's art cache, found without reading any manifest.
pub fn art_cache() -> Option<PathBuf> {
    root().as_deref().map(cache_in)
}

#[cfg(windows)]
fn registry_root() -> Option<PathBuf> {
    use crate::registry::{self, HKEY_CURRENT_USER};
    registry::string(HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath").map(PathBuf::from)
}

#[cfg(not(windows))]
fn registry_root() -> Option<PathBuf> {
    None
}

fn root() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    [
        registry_root(),
        Some(PathBuf::from(r"C:\Program Files (x86)\Steam")),
        Some(home.join("Library/Application Support/Steam")),
    ]
    .into_iter()
    .flatten()
    .find(|p| p.join("steamapps").is_dir())
}

/// Steam's own tools that show up as installed apps.
fn is_tool(app_id: u32, name: &str) -> bool {
    app_id == 228980
        || ["Steamworks Common", "Proton", "Steam Linux Runtime"]
            .iter()
            .any(|p| name.starts_with(p))
}

pub fn libraries(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![root.to_path_buf()];
    if let Ok(text) = fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf")) {
        let doc = vdf::parse(&text);
        if let Some(folders) = doc.get("libraryfolders") {
            for (_, folder) in folders.entries() {
                if let Some(path) = folder.str("path") {
                    let path = PathBuf::from(path);
                    let known = out
                        .iter()
                        .any(|p| p.to_string_lossy().eq_ignore_ascii_case(&path.to_string_lossy()));
                    if !known {
                        out.push(path);
                    }
                }
            }
        }
    }
    out
}

pub fn parse_manifest(text: &str, library: &Path) -> Option<SteamApp> {
    let doc = vdf::parse(text);
    let state = doc.get("AppState")?;
    let app_id: u32 = state.str("appid")?.parse().ok()?;
    let name = state.str("name")?.to_string();
    let flags: u32 = state.str("StateFlags").and_then(|f| f.parse().ok()).unwrap_or(0);
    if flags & 4 == 0 || is_tool(app_id, &name) {
        return None;
    }
    Some(SteamApp {
        app_id,
        name,
        install_dir: library.join("steamapps").join("common").join(state.str("installdir")?),
        size_bytes: state.str("SizeOnDisk").and_then(|s| s.parse().ok()).unwrap_or(0),
        cover: format!("{CDN}/{app_id}/library_600x900_2x.jpg"),
        hero: format!("{CDN}/{app_id}/library_hero.jpg"),
        logo: format!("{CDN}/{app_id}/logo.png"),
    })
}

/// Prefers the copy Steam already downloaded: `<id>/…/<file>` (new) or `<id>_<file>` (old).
fn local_art(cache: &Path, app_id: u32, file: &str) -> Option<String> {
    let flat = cache.join(format!("{app_id}_{file}"));
    if flat.is_file() {
        return Some(flat.to_string_lossy().into_owned());
    }
    let dir = cache.join(app_id.to_string());
    let direct = dir.join(file);
    if direct.is_file() {
        return Some(direct.to_string_lossy().into_owned());
    }
    fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path().join(file))
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
}

pub fn scan() -> Option<Steam> {
    let root = root()?;
    let cache = cache_in(&root);
    let mut apps = HashMap::new();
    for library in libraries(&root) {
        let Ok(dir) = fs::read_dir(library.join("steamapps")) else { continue };
        for entry in dir.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Some(mut app) = fs::read_to_string(entry.path()).ok().and_then(|t| parse_manifest(&t, &library)) else {
                continue;
            };
            if let Some(p) = local_art(&cache, app.app_id, "library_600x900.jpg") {
                app.cover = p;
            }
            if let Some(p) = local_art(&cache, app.app_id, "library_hero.jpg") {
                app.hero = p;
            }
            if let Some(p) = local_art(&cache, app.app_id, "logo.png") {
                app.logo = p;
            }
            apps.insert(app.app_id, app);
        }
    }
    Some(Steam { root, apps })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#""AppState"
{
	"appid"		"620"
	"name"		"Portal 2"
	"StateFlags"		"4"
	"installdir"		"Portal 2"
	"SizeOnDisk"		"12884901888"
}"#;

    #[test]
    fn reads_an_installed_game() {
        let app = parse_manifest(MANIFEST, Path::new("D:/SteamLibrary")).unwrap();
        assert_eq!(app.app_id, 620);
        assert_eq!(app.size_bytes, 12_884_901_888);
        assert_eq!(app.install_dir, Path::new("D:/SteamLibrary/steamapps/common/Portal 2"));
        assert!(app.cover.ends_with("/620/library_600x900_2x.jpg"));
    }

    #[test]
    fn skips_downloads_in_progress_and_steam_tools() {
        let downloading = MANIFEST.replace("\"StateFlags\"\t\t\"4\"", "\"StateFlags\"\t\t\"1026\"");
        assert!(parse_manifest(&downloading, Path::new("D:/")).is_none());
        let tool = MANIFEST.replace("Portal 2\"\n", "Proton 9.0\"\n");
        assert!(parse_manifest(&tool, Path::new("D:/")).is_none());
    }

    #[test]
    fn finds_cached_art_in_both_layouts() {
        let dir = std::env::temp_dir().join(format!("beanbag-art-{}", std::process::id()));
        fs::create_dir_all(dir.join("620").join("abc123")).unwrap();
        fs::write(dir.join("620").join("abc123").join("logo.png"), b"x").unwrap();
        fs::write(dir.join("440_library_hero.jpg"), b"x").unwrap();
        assert!(local_art(&dir, 620, "logo.png").unwrap().ends_with("logo.png"));
        assert!(local_art(&dir, 440, "library_hero.jpg").unwrap().ends_with("440_library_hero.jpg"));
        assert!(local_art(&dir, 620, "library_hero.jpg").is_none());
        fs::remove_dir_all(dir).unwrap();
    }
}
