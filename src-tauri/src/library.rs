//! Beanbag's own library, in layers that never overwrite each other:
//! `library.json` (the Playnite import), `user.json` (games added and edits made in
//! Beanbag), `fixes.json` (art and text found online), `stats.json` (playtime).
//! Games are checked against the disk, and Steam games installed since are added.

use crate::art::{self, Kind, Need};
use crate::catalogs::{self, Traits};
use crate::names;
use crate::steam::Steam;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

pub const LIBRARY_FILE: &str = "library.json";
pub const USER_FILE: &str = "user.json";
pub const STATS_FILE: &str = "stats.json";

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Launch {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub exe: Option<String>,
    #[serde(default)]
    pub args: String,
    #[serde(default)]
    pub working_dir: Option<String>,
}

#[derive(Deserialize, Serialize, Default, Clone, Debug, PartialEq)]
pub struct StoredArt {
    pub cover: Option<String>,
    pub background: Option<String>,
    pub logo: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct StoredGame {
    pub id: String,
    pub name: String,
    pub sort_name: String,
    pub source: String,
    pub platform: String,
    pub description: String,
    pub size_bytes: u64,
    pub install_dir: String,
    pub steam_app_id: Option<u32>,
    pub launch: Launch,
    pub art: StoredArt,
    pub genres: Vec<String>,
    /// Playnite categories; shown as catalogs.
    pub categories: Vec<String>,
}

/// What a person changed in Beanbag. Only the fields that are set win.
#[derive(Deserialize, Serialize, Default, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Override {
    pub name: Option<String>,
    pub description: Option<String>,
    pub genres: Option<Vec<String>>,
    pub catalogs: Option<Vec<String>>,
    pub launch: Option<Launch>,
    pub install_dir: Option<String>,
    pub art: StoredArt,
    pub hidden: bool,
}

#[derive(Deserialize, Serialize, Default, Clone, Debug)]
#[serde(default)]
pub struct User {
    /// Games added in Beanbag.
    pub games: Vec<StoredGame>,
    pub overrides: HashMap<String, Override>,
}

pub fn read_user(data: &Path) -> User {
    fs::read_to_string(data.join(USER_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(strip_bom(&t)).ok())
        .unwrap_or_default()
}

pub fn write_user(data: &Path, user: &User) -> std::io::Result<()> {
    let tmp = data.join(format!("{USER_FILE}.tmp"));
    fs::write(&tmp, serde_json::to_vec_pretty(user)?)?;
    fs::rename(tmp, data.join(USER_FILE))
}

#[derive(Deserialize)]
struct Stored {
    #[serde(default)]
    games: Vec<StoredGame>,
}

#[derive(Deserialize, Serialize, Default, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stat {
    #[serde(default)]
    pub last_played: i64,
    /// Seconds.
    #[serde(default)]
    pub playtime: u64,
    #[serde(default)]
    pub play_count: u64,
}

/// What the page sees.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub name: String,
    pub sort_name: String,
    pub last_played: i64,
    pub playtime: u64,
    pub size_bytes: u64,
    pub source: String,
    pub platform: String,
    pub description: String,
    pub genres: Vec<String>,
    pub catalogs: Vec<String>,
    pub hidden: bool,
    /// Added in Beanbag rather than imported.
    pub added: bool,
    pub install_dir: String,
    /// Steam review count, or LaunchBox rating count.
    pub popularity: u64,
    /// Unix seconds; 0 when unknown.
    pub released: i64,
    pub cover: Option<String>,
    pub background: Option<String>,
    pub logo: Option<String>,
}

/// A game plus what Beanbag needs to start and follow it.
#[derive(Clone, Debug)]
pub struct Entry {
    pub game: Game,
    pub launch: Launch,
    pub install_dir: PathBuf,
    pub steam_app_id: Option<u32>,
    /// Tags, player modes or a player count are known, so the rows can place it.
    pub has_traits: bool,
}

impl Entry {
    fn art(&self, kind: Kind) -> Option<&String> {
        match kind {
            Kind::Cover => self.game.cover.as_ref(),
            Kind::Background => self.game.background.as_ref(),
            Kind::Logo => self.game.logo.as_ref(),
        }
    }
}

/// Games with art that is missing or only on the web, or with no description or genres.
pub fn needs(entries: &[Entry]) -> Vec<Need> {
    entries
        .iter()
        .filter(|e| !e.game.hidden)
        .filter_map(|e| {
            let kinds: Vec<Kind> = Kind::ALL
                .into_iter()
                .filter(|&k| e.art(k).is_none_or(|a| a.starts_with("http")))
                .collect();
            let description = e.game.description.is_empty();
            let genres = e.game.genres.is_empty();
            let traits = !e.has_traits;
            (!kinds.is_empty() || description || genres || traits).then(|| Need {
                id: e.game.id.clone(),
                name: e.game.name.clone(),
                install_dir: e.install_dir.to_string_lossy().into_owned(),
                platform: e.game.platform.clone(),
                app_id: e.steam_app_id,
                kinds,
                description,
                genres,
                traits,
            })
        })
        .collect()
}

#[derive(Serialize, Debug, Default)]
pub struct Library {
    pub found: bool,
    pub games: Vec<Game>,
    /// The big-screen rows, in order.
    pub rows: Vec<&'static str>,
}

fn strip_bom(text: &str) -> &str {
    text.trim_start_matches('\u{feff}')
}

pub fn read_stats(data: &Path) -> HashMap<String, Stat> {
    fs::read_to_string(data.join(STATS_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(strip_bom(&t)).ok())
        .unwrap_or_default()
}

fn write_stats(data: &Path, stats: &HashMap<String, Stat>) -> std::io::Result<()> {
    let tmp = data.join(format!("{STATS_FILE}.tmp"));
    fs::write(&tmp, serde_json::to_vec(stats)?)?;
    fs::rename(tmp, data.join(STATS_FILE))
}

/// A launch counts at once for "Recent"; playtime arrives when the game closes.
pub fn record_launch(data: &Path, id: &str, now: i64) -> std::io::Result<()> {
    let mut stats = read_stats(data);
    let s = stats.entry(id.to_string()).or_default();
    s.last_played = now;
    s.play_count += 1;
    write_stats(data, &stats)
}

pub fn record_playtime(data: &Path, id: &str, seconds: u64) -> std::io::Result<()> {
    let mut stats = read_stats(data);
    stats.entry(id.to_string()).or_default().playtime += seconds;
    write_stats(data, &stats)
}

/// Relative art paths live under the data folder; absolute paths and URLs pass through.
fn art_path(data: &Path, value: Option<String>) -> Option<String> {
    let v = value.filter(|v| !v.is_empty())?;
    if v.starts_with("http://") || v.starts_with("https://") || Path::new(&v).is_absolute() {
        return Some(v);
    }
    Some(data.join(v).to_string_lossy().into_owned())
}

/// Still on disk? A Steam game must still be installed in Steam; a game with an exe needs the exe.
fn present(game: &StoredGame, steam: Option<&Steam>) -> bool {
    if let (Some(app_id), Some(steam)) = (game.steam_app_id, steam) {
        return steam.apps.contains_key(&app_id);
    }
    match &game.launch.exe {
        Some(exe) => Path::new(exe).is_file(),
        None => game.launch.url.is_some(),
    }
}

/// A folder as a comparable key: one slash style, no trailing slash, any case.
fn folder_key(path: &str) -> String {
    path.replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

/// Every game: the import and games added in Beanbag, then Steam's, then the other
/// launchers' (`others`), each skipped when its folder is already in the library.
pub fn load(data: &Path, steam: Option<&Steam>, others: &[StoredGame]) -> (Library, Vec<Entry>) {
    let stored: Option<Stored> = fs::read_to_string(data.join(LIBRARY_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(strip_bom(&t)).ok());
    let user = read_user(data);
    let stats = read_stats(data);
    let fixes = art::read_fixes(data);
    let found = stored.is_some() || steam.is_some() || !user.games.is_empty() || !others.is_empty();
    let mut entries = Vec::new();
    let mut steam_ids = HashSet::new();

    let added: HashSet<&str> = user.games.iter().map(|g| g.id.as_str()).collect();
    let imported = stored.map(|s| s.games).unwrap_or_default();
    for g in imported.iter().chain(user.games.iter()) {
        if !present(g, steam) {
            continue;
        }
        if let (Some(id), Some(_)) = (g.steam_app_id, steam) {
            steam_ids.insert(id);
        }
        let mut install_dir = g.install_dir.clone();
        if let (Some(id), Some(steam)) = (g.steam_app_id, steam) {
            install_dir = steam.apps[&id].install_dir.to_string_lossy().into_owned();
        }
        entries.push(entry(data, g.clone(), install_dir, added.contains(g.id.as_str()), &user, &fixes, &stats, None));
    }

    for app in steam.map(|s| s.apps.values()).into_iter().flatten() {
        if steam_ids.contains(&app.app_id) {
            continue;
        }
        let g = StoredGame {
            id: format!("steam-{}", app.app_id),
            name: app.name.clone(),
            source: "Steam".into(),
            size_bytes: app.size_bytes,
            steam_app_id: Some(app.app_id),
            launch: Launch { url: Some(format!("steam://rungameid/{}", app.app_id)), ..Default::default() },
            ..Default::default()
        };
        let web = StoredArt { cover: Some(app.cover.clone()), background: Some(app.hero.clone()), logo: Some(app.logo.clone()) };
        let dir = app.install_dir.to_string_lossy().into_owned();
        entries.push(entry(data, g, dir, false, &user, &fixes, &stats, Some(web)));
    }

    let mut known: HashSet<String> = entries.iter().map(|e| folder_key(&e.install_dir.to_string_lossy())).collect();
    for g in others {
        if g.install_dir.is_empty() || !known.insert(folder_key(&g.install_dir)) {
            continue;
        }
        entries.push(entry(data, g.clone(), g.install_dir.clone(), false, &user, &fixes, &stats, None));
    }

    let library = Library { found, games: entries.iter().map(|e| e.game.clone()).collect(), rows: catalogs::ORDER.to_vec() };
    (library, entries)
}

/// One game with every layer applied: edits, then the import, then fixes, then Steam.
#[allow(clippy::too_many_arguments)]
fn entry(
    data: &Path,
    g: StoredGame,
    install_dir: String,
    added: bool,
    user: &User,
    fixes: &HashMap<String, art::Fix>,
    stats: &HashMap<String, Stat>,
    steam_art: Option<StoredArt>,
) -> Entry {
    let edit = user.overrides.get(&g.id).cloned().unwrap_or_default();
    let fix = fixes.get(&g.id).cloned().unwrap_or_default();
    let stat = stats.get(&g.id).copied().unwrap_or_default();
    let steam_art = steam_art.unwrap_or_default();
    let fixed = |kind: Kind| art::on_disk(data, &g.id, kind).map(|p| p.to_string_lossy().into_owned());
    let pick = |mine: Option<String>, theirs: Option<String>, kind: Kind, web: Option<String>| {
        art_path(data, mine).or_else(|| art_path(data, theirs)).or_else(|| fixed(kind)).or(web)
    };
    let name = edit.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or(g.name.clone());
    let genres = edit.genres.clone().unwrap_or_else(|| {
        let own = names::genres(&g.genres);
        if own.is_empty() { names::genres(&fix.genres) } else { own }
    });
    let description = edit
        .description
        .clone()
        .unwrap_or(if g.description.is_empty() { fix.description.clone() } else { g.description.clone() });
    let install_dir = edit.install_dir.clone().unwrap_or(install_dir);
    // Rows come from how the game is played; an edit in desktop mode replaces them,
    // and Playnite categories add to them.
    let catalogs = edit.catalogs.as_ref().map(|c| c.iter().map(|n| catalogs::renamed(n).to_string()).collect()).unwrap_or_else(|| {
        let traits = Traits {
            tags: &fix.tags,
            players: &fix.players,
            features: &fix.features,
            max_players: fix.max_players,
            platform: &g.platform,
            genres: &genres,
        };
        let mut all: Vec<String> = catalogs::classify(&traits).into_iter().map(String::from).collect();
        let categories: Vec<String> = g.categories.iter().map(|c| catalogs::renamed(c).to_string()).collect();
        all.extend(categories.into_iter().filter(|c| !all.contains(c)).collect::<Vec<_>>());
        all
    });
    Entry {
        game: Game {
            sort_name: if edit.name.is_some() || g.sort_name.is_empty() { name.clone() } else { g.sort_name.clone() },
            name,
            last_played: stat.last_played,
            playtime: stat.playtime,
            size_bytes: g.size_bytes,
            source: g.source.clone(),
            platform: g.platform.clone(),
            description,
            genres,
            catalogs,
            hidden: edit.hidden,
            added,
            install_dir: install_dir.clone(),
            popularity: fix.popularity,
            released: fix.released,
            cover: pick(edit.art.cover.clone(), g.art.cover.clone(), Kind::Cover, steam_art.cover),
            background: pick(edit.art.background.clone(), g.art.background.clone(), Kind::Background, steam_art.background),
            logo: pick(edit.art.logo.clone(), g.art.logo.clone(), Kind::Logo, steam_art.logo),
            id: g.id.clone(),
        },
        launch: edit.launch.clone().unwrap_or(g.launch.clone()),
        install_dir: PathBuf::from(install_dir),
        steam_app_id: g.steam_app_id,
        has_traits: fix.knows_traits(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steam::SteamApp;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("beanbag-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn steam_with(ids: &[u32]) -> Steam {
        let apps = ids
            .iter()
            .map(|&id| {
                let app = SteamApp {
                    app_id: id,
                    name: format!("Steam {id}"),
                    install_dir: PathBuf::from(format!("/steam/common/{id}")),
                    size_bytes: 1,
                    cover: "https://c".into(),
                    hero: "https://h".into(),
                    logo: "https://l".into(),
                };
                (id, app)
            })
            .collect();
        Steam { root: PathBuf::from("/steam"), apps }
    }

    #[test]
    fn keeps_what_is_installed_and_adds_new_steam_games() {
        let data = temp("load");
        let exe = data.join("game.exe");
        fs::write(&exe, b"").unwrap();
        let library = serde_json::json!({ "games": [
            { "id": "a", "name": "On Disk", "launch": { "exe": exe }, "art": { "cover": "art/a/cover.jpg" } },
            { "id": "b", "name": "Gone", "launch": { "exe": data.join("missing.exe") } },
            { "id": "c", "name": "Still In Steam", "steamAppId": 620, "launch": { "url": "steam://rungameid/620" } },
            { "id": "d", "name": "Removed From Steam", "steamAppId": 440, "launch": { "url": "steam://rungameid/440" } },
            { "id": "e", "name": "Epic", "launch": { "url": "com.epicgames.launcher://apps/x" } }
        ]});
        fs::write(data.join(LIBRARY_FILE), library.to_string()).unwrap();
        record_launch(&data, "a", 1_700_000_000).unwrap();
        record_playtime(&data, "a", 90).unwrap();

        let steam = steam_with(&[620, 1145360]);
        let (lib, entries) = load(&data, Some(&steam), &[]);
        let mut ids: Vec<_> = lib.games.iter().map(|g| g.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, ["a", "c", "e", "steam-1145360"]);

        let a = &lib.games.iter().find(|g| g.id == "a").unwrap();
        assert_eq!((a.last_played, a.playtime), (1_700_000_000, 90));
        assert_eq!(a.sort_name, "On Disk");
        assert!(a.cover.as_deref().unwrap().ends_with("cover.jpg"));
        assert!(Path::new(a.cover.as_deref().unwrap()).is_absolute());

        let c = entries.iter().find(|e| e.game.id == "c").unwrap();
        assert_eq!(c.install_dir, Path::new("/steam/common/620"));
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn asks_for_art_that_is_missing_or_only_on_the_web() {
        let data = temp("needs");
        let library = serde_json::json!({ "games": [
            { "id": "full", "name": "Full", "description": "d", "genres": ["Shooter"], "launch": { "url": "x://" },
              "art": { "cover": "/c.jpg", "background": "/b.jpg", "logo": "/l.png" } },
            { "id": "bare", "name": "Bare", "launch": { "url": "x://" }, "art": { "cover": "/c.jpg" } }
        ]});
        fs::write(data.join(LIBRARY_FILE), library.to_string()).unwrap();
        fs::write(data.join("fixes.json"), format!(r#"{{"full":{{"tags":["Indie"],"pass":{}}}}}"#, art::PASS)).unwrap();
        let (_, entries) = load(&data, Some(&steam_with(&[620])), &[]);
        let needs = needs(&entries);
        let bare = needs.iter().find(|n| n.id == "bare").unwrap();
        assert_eq!(bare.kinds, [Kind::Background, Kind::Logo]);
        assert!(bare.description);
        let steam = needs.iter().find(|n| n.id == "steam-620").unwrap();
        assert_eq!(steam.kinds, Kind::ALL);
        assert_eq!(steam.app_id, Some(620));
        assert!(!needs.iter().any(|n| n.id == "full"));
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn edits_beat_the_import_and_fixes_fill_the_gaps() {
        let data = temp("layers");
        let library = serde_json::json!({ "games": [
            { "id": "a", "name": "Imported", "genres": ["Role-playing (RPG)"], "categories": ["Couch"], "launch": { "url": "x://" } },
            { "id": "b", "name": "Bare", "launch": { "url": "x://" } }
        ]});
        fs::write(data.join(LIBRARY_FILE), library.to_string()).unwrap();
        let mut user = User::default();
        user.overrides.insert("a".into(), Override { name: Some("Renamed".into()), hidden: true, ..Default::default() });
        user.games.push(StoredGame { id: "mine".into(), name: "Added".into(), launch: Launch { url: Some("y://".into()), ..Default::default() }, ..Default::default() });
        write_user(&data, &user).unwrap();
        let fixes = serde_json::json!({ "b": { "description": "Found online", "genres": ["Role-Playing", "Shooter"], "checked": 1 } });
        fs::write(data.join("fixes.json"), fixes.to_string()).unwrap();

        let (lib, _) = load(&data, None, &[]);
        let get = |id: &str| lib.games.iter().find(|g| g.id == id).unwrap().clone();
        let a = get("a");
        assert_eq!((a.name.as_str(), a.sort_name.as_str(), a.hidden), ("Renamed", "Renamed", true));
        assert_eq!((a.genres, a.catalogs), (vec!["RPG".to_string()], vec!["Couch".to_string()]));
        let b = get("b");
        assert_eq!(b.description, "Found online");
        assert_eq!(b.genres, ["RPG", "Shooter"]);
        assert!(get("mine").added);
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn catalogs_saved_under_the_old_row_names_follow_the_rename() {
        let data = temp("renamed");
        let library = serde_json::json!({ "games": [
            { "id": "a", "name": "Edited", "launch": { "url": "x://" } },
            { "id": "b", "name": "Imported", "categories": ["Same screen", "Couch picks"], "launch": { "url": "x://" } }
        ]});
        fs::write(data.join(LIBRARY_FILE), library.to_string()).unwrap();
        let mut user = User::default();
        user.overrides.insert("a".into(), Override { catalogs: Some(vec!["AAA story".into(), "Multiplayer".into()]), ..Default::default() });
        write_user(&data, &user).unwrap();
        let (lib, _) = load(&data, None, &[]);
        let get = |id: &str| lib.games.iter().find(|g| g.id == id).unwrap().catalogs.clone();
        assert_eq!(get("a"), ["Story", "Online"]);
        assert_eq!(get("b"), ["Couch co-op", "Couch picks"]);
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn adds_games_from_other_launchers_once() {
        let data = temp("launchers");
        let library = serde_json::json!({ "games": [
            { "id": "pn", "name": "From Playnite", "installDir": "D:\\Epic\\NightGarden", "launch": { "url": "com.epicgames.launcher://x" } }
        ]});
        fs::write(data.join(LIBRARY_FILE), library.to_string()).unwrap();
        let game = |id: &str, dir: &str| StoredGame {
            id: id.into(),
            name: id.into(),
            install_dir: dir.into(),
            launch: Launch { url: Some("x://".into()), ..Default::default() },
            ..Default::default()
        };
        let others = [game("epic-a", "d:/epic/nightgarden/"), game("gog-1", r"D:\GOG\Quiet Harbour"), game("gog-2", r"d:\gog\quiet harbour")];
        let (lib, _) = load(&data, None, &others);
        let mut ids: Vec<_> = lib.games.iter().map(|g| g.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, ["gog-1", "pn"], "one entry per folder, whoever found it first");
        let alone = temp("launchers-only");
        assert!(load(&alone, None, &others).0.found, "launcher games alone are a library");
        fs::remove_dir_all(alone).unwrap();
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn no_import_and_no_steam_means_no_library() {
        let data = temp("empty");
        let (lib, _) = load(&data, None, &[]);
        assert!(!lib.found && lib.games.is_empty());
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn stats_add_up_across_sessions() {
        let data = temp("stats");
        record_launch(&data, "x", 10).unwrap();
        record_playtime(&data, "x", 60).unwrap();
        record_launch(&data, "x", 20).unwrap();
        record_playtime(&data, "x", 30).unwrap();
        let s = read_stats(&data)["x"];
        assert_eq!(s, Stat { last_played: 20, playtime: 90, play_count: 2 });
        fs::remove_dir_all(data).unwrap();
    }
}
