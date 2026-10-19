//! Games installed through other launchers, read from each launcher's own records, so they
//! show up without Playnite: Epic Games (its install manifests) and GOG (the registry).

use crate::library::{Launch, StoredGame};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Every game the other launchers have installed.
pub fn scan() -> Vec<StoredGame> {
    let mut out = epic();
    out.extend(gog());
    out
}

/// Ids become folder names under `art/`, so only plain names pass: no separators, no "..".
fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// "Rocket League®" → "Rocket League".
fn clean(name: &str) -> String {
    name.replace(['™', '®', '©'], "").trim().to_string()
}

// ── Epic Games ───────────────────────────────────────────────────

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct EpicManifest {
    display_name: String,
    app_name: String,
    catalog_namespace: String,
    catalog_item_id: String,
    install_location: String,
    launch_executable: String,
    app_categories: Vec<String>,
    main_game_app_name: String,
    install_size: u64,
    #[serde(rename = "bIsIncompleteInstall")]
    incomplete: bool,
}

/// One Epic manifest as a game; None for DLC, apps that are not games (engines, mod kits)
/// and installs that have not finished.
fn epic_game(text: &str) -> Option<StoredGame> {
    let m: EpicManifest = serde_json::from_str(text).ok()?;
    let is_game = m.app_categories.iter().any(|c| c == "games");
    let dlc = !m.main_game_app_name.is_empty() && m.main_game_app_name != m.app_name;
    if !is_game || dlc || m.incomplete || m.launch_executable.is_empty() || !safe_id(&m.app_name) {
        return None;
    }
    let url = format!(
        "com.epicgames.launcher://apps/{}%3A{}%3A{}?action=launch&silent=true",
        m.catalog_namespace, m.catalog_item_id, m.app_name
    );
    Some(StoredGame {
        id: format!("epic-{}", m.app_name),
        name: clean(&m.display_name),
        source: "Epic".into(),
        install_dir: m.install_location,
        size_bytes: m.install_size,
        launch: Launch { url: Some(url), ..Default::default() },
        ..Default::default()
    })
}

fn epic() -> Vec<StoredGame> {
    let root = std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
    let manifests = root.join("Epic").join("EpicGamesLauncher").join("Data").join("Manifests");
    let mut out: Vec<StoredGame> = Vec::new();
    for entry in fs::read_dir(manifests).into_iter().flatten().flatten() {
        if entry.path().extension().is_none_or(|e| e != "item") {
            continue;
        }
        let Some(game) = fs::read_to_string(entry.path()).ok().and_then(|t| epic_game(&t)) else { continue };
        // A game can leave more than one manifest behind; the first one that is still on disk counts.
        if Path::new(&game.install_dir).is_dir() && !out.iter().any(|o| o.id == game.id) {
            out.push(game);
        }
    }
    out
}

// ── GOG ──────────────────────────────────────────────────────────

/// One GOG install from its registry values; None for DLC and extras, which name the game
/// they belong to, and for anything without a program to start. Only Windows has a
/// registry to read it from; the tests use it everywhere.
#[cfg(any(windows, test))]
fn gog_game(get: impl Fn(&str) -> Option<String>) -> Option<StoredGame> {
    if get("dependsOn").is_some_and(|parent| !parent.trim().is_empty()) {
        return None;
    }
    let id = get("gameID").filter(|id| id.chars().all(|c| c.is_ascii_digit()) && !id.is_empty())?;
    let exe = get("exe").filter(|e| !e.trim().is_empty())?;
    let dir = get("path").unwrap_or_default();
    let working_dir = get("workingDir").filter(|w| !w.trim().is_empty()).or_else(|| Some(dir.clone()).filter(|d| !d.is_empty()));
    Some(StoredGame {
        id: format!("gog-{id}"),
        name: clean(&get("gameName")?),
        source: "GOG".into(),
        install_dir: dir,
        launch: Launch { exe: Some(exe), args: get("launchParam").unwrap_or_default(), working_dir, url: None },
        ..Default::default()
    })
}

#[cfg(windows)]
fn gog() -> Vec<StoredGame> {
    use crate::registry::{self, HKEY_LOCAL_MACHINE};
    const GAMES: &str = r"SOFTWARE\WOW6432Node\GOG.com\Games";
    registry::subkeys(HKEY_LOCAL_MACHINE, GAMES)
        .into_iter()
        .filter_map(|key| gog_game(|value| registry::string(HKEY_LOCAL_MACHINE, &format!(r"{GAMES}\{key}"), value)))
        .filter(|g| g.launch.exe.as_deref().is_some_and(|exe| Path::new(exe).is_file()))
        .collect()
}

#[cfg(not(windows))]
fn gog() -> Vec<StoredGame> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPIC: &str = r#"{
        "DisplayName": "Night Garden®", "AppName": "Lily", "CatalogNamespace": "ns1", "CatalogItemId": "item1",
        "InstallLocation": "D:\\Epic\\NightGarden", "LaunchExecutable": "Bin/Game.exe",
        "AppCategories": ["public", "games", "applications"], "MainGameAppName": "Lily",
        "bIsIncompleteInstall": false, "InstallSize": 42
    }"#;

    #[test]
    fn reads_an_epic_game() {
        let g = epic_game(EPIC).unwrap();
        assert_eq!((g.id.as_str(), g.name.as_str(), g.source.as_str()), ("epic-Lily", "Night Garden", "Epic"));
        assert_eq!(g.launch.url.as_deref(), Some("com.epicgames.launcher://apps/ns1%3Aitem1%3ALily?action=launch&silent=true"));
        assert_eq!((g.install_dir.as_str(), g.size_bytes), (r"D:\Epic\NightGarden", 42));
    }

    #[test]
    fn skips_epic_dlc_tools_and_unfinished_installs() {
        assert!(epic_game(&EPIC.replace(r#""MainGameAppName": "Lily""#, r#""MainGameAppName": "Base""#)).is_none());
        assert!(epic_game(&EPIC.replace(r#""games", "#, "")).is_none());
        assert!(epic_game(&EPIC.replace(r#""bIsIncompleteInstall": false"#, r#""bIsIncompleteInstall": true"#)).is_none());
        assert!(epic_game(&EPIC.replace(r#""LaunchExecutable": "Bin/Game.exe""#, r#""LaunchExecutable": """#)).is_none());
        assert!(epic_game("not a manifest").is_none());
        assert!(epic_game(&EPIC.replace(r#""AppName": "Lily""#, r#""AppName": "../../x""#)).is_none(), "an id must not leave art/");
    }

    fn values<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| pairs.iter().find(|(n, _)| *n == name).map(|(_, v)| v.to_string())
    }

    #[test]
    fn reads_a_gog_game() {
        let g = gog_game(values(&[
            ("gameID", "1207"),
            ("gameName", "Quiet Harbour™"),
            ("exe", r"D:\GOG\Quiet Harbour\harbour.exe"),
            ("path", r"D:\GOG\Quiet Harbour"),
            ("dependsOn", ""),
        ]))
        .unwrap();
        assert_eq!((g.id.as_str(), g.name.as_str(), g.source.as_str()), ("gog-1207", "Quiet Harbour", "GOG"));
        assert_eq!(g.launch.exe.as_deref(), Some(r"D:\GOG\Quiet Harbour\harbour.exe"));
        assert_eq!(g.launch.working_dir.as_deref(), Some(r"D:\GOG\Quiet Harbour"), "starts in its own folder");
    }

    #[test]
    fn skips_gog_extras_and_entries_without_a_program() {
        assert!(gog_game(values(&[("gameID", "2"), ("gameName", "Soundtrack"), ("exe", "x.exe"), ("dependsOn", "1207")])).is_none());
        assert!(gog_game(values(&[("gameID", "3"), ("gameName", "No program"), ("path", r"D:\GOG\x")])).is_none());
        assert!(gog_game(values(&[("gameID", r"..\x"), ("gameName", "Odd"), ("exe", "x.exe")])).is_none(), "an id must not leave art/");
    }

    /// Lists what this PC's launchers have installed. Run by hand on Windows:
    /// cargo test lists_installed_games -- --ignored --nocapture
    #[test]
    #[ignore]
    fn lists_installed_games() {
        for g in scan() {
            println!("{:<5} {} | {} | {}", g.source, g.name, g.install_dir, g.launch.url.or(g.launch.exe).unwrap_or_default());
        }
    }
}
