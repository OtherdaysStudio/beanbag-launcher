//! Desktop mode: adding games, scanning folders, editing details and art, and finding
//! metadata online. Every change lands in `user.json`, so a new import never undoes it.

use crate::art::{self, Kind, Store};
use crate::library::{self, Launch, StoredGame, User};
use crate::{launchbox, names, sgdb};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Fields a person changed; only the ones present change.
#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Patch {
    pub name: Option<String>,
    pub description: Option<String>,
    pub genres: Option<Vec<String>>,
    pub catalogs: Option<Vec<String>>,
    pub hidden: Option<bool>,
}

pub fn apply(user: &mut User, id: &str, patch: Patch) {
    let o = user.overrides.entry(id.to_string()).or_default();
    if let Some(v) = patch.name {
        o.name = Some(v.trim().to_string()).filter(|v| !v.is_empty());
    }
    if let Some(v) = patch.description {
        o.description = Some(v);
    }
    if let Some(v) = patch.genres {
        o.genres = Some(tidy(v));
    }
    if let Some(v) = patch.catalogs {
        o.catalogs = Some(tidy(v));
    }
    if let Some(v) = patch.hidden {
        o.hidden = v;
    }
}

/// Trimmed, non-empty, no duplicates ignoring case.
fn tidy(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty() && seen.insert(v.to_lowercase()))
        .collect()
}

fn stamp() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis())
}

/// Copies or downloads an image into the game's art folder and points the edit at it.
/// The file name changes each time, so the page never shows a cached old image.
pub fn set_art(data: &Path, user: &mut User, id: &str, kind: Kind, source: &str) -> Result<(), String> {
    let dir = data.join("art").join(id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = format!("custom-{}-{}.{}", kind.stem(), stamp(), art::extension(source));
    let dest = dir.join(&file);
    if source.starts_with("http://") || source.starts_with("https://") {
        if !Store::new().download(source, &dest) {
            return Err("That image could not be downloaded.".into());
        }
    } else {
        fs::copy(source, &dest).map_err(|e| e.to_string())?;
    }
    // Earlier custom images of this kind are no longer used.
    let prefix = format!("custom-{}-", kind.stem());
    for old in fs::read_dir(&dir).into_iter().flatten().flatten() {
        let name = old.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && name != file {
            let _ = fs::remove_file(old.path());
        }
    }
    let rel = Some(format!("art/{id}/{file}"));
    let o = user.overrides.entry(id.to_string()).or_default();
    match kind {
        Kind::Cover => o.art.cover = rel,
        Kind::Background => o.art.background = rel,
        Kind::Logo => o.art.logo = rel,
    }
    Ok(())
}

/// Removes a game added in Beanbag; an imported game is hidden instead.
pub fn remove(data: &Path, user: &mut User, id: &str) {
    let before = user.games.len();
    user.games.retain(|g| g.id != id);
    if user.games.len() < before {
        user.overrides.remove(id);
        let _ = fs::remove_dir_all(data.join("art").join(id));
    } else {
        user.overrides.entry(id.to_string()).or_default().hidden = true;
    }
}

/// Folder names that hold an exe but say nothing about the game.
const PLUMBING: [&str; 9] = ["bin", "binaries", "win64", "win32", "x64", "x86", "64", "32", "game"];

/// "C:\Games\Hades\x64\Hades.exe" → "C:\Games\Hades": the folder that names the game.
pub fn install_root(exe: &Path) -> PathBuf {
    let mut dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    while let Some(name) = dir.file_name().map(|n| n.to_string_lossy().to_lowercase()) {
        if !PLUMBING.contains(&name.as_str()) {
            break;
        }
        match dir.parent() {
            Some(p) if p.components().count() > 1 => dir = p.to_path_buf(),
            _ => break,
        }
    }
    dir
}

fn game_name(exe: &Path) -> String {
    let root = install_root(exe);
    names::from_folder(&root.to_string_lossy())
        .or_else(|| exe.file_stem().map(|s| s.to_string_lossy().replace(['_', '.'], " ")))
        .unwrap_or_else(|| "New game".into())
}

/// Adds games by their programs. Skips programs already in the library. Returns new ids.
pub fn add(user: &mut User, existing: &[PathBuf], exes: &[String]) -> Vec<String> {
    let key = |p: &Path| p.to_string_lossy().to_lowercase().replace('/', "\\");
    let mut known: HashSet<String> = existing.iter().map(|p| key(p)).collect();
    known.extend(user.games.iter().filter_map(|g| g.launch.exe.as_ref()).map(|e| key(Path::new(e))));
    let mut added = Vec::new();
    for (i, exe) in exes.iter().enumerate() {
        let path = Path::new(exe);
        if !known.insert(key(path)) {
            continue;
        }
        let id = format!("user-{:x}{i:02x}", stamp());
        let name = game_name(path);
        user.games.push(StoredGame {
            id: id.clone(),
            sort_name: name.clone(),
            name,
            install_dir: install_root(path).to_string_lossy().into_owned(),
            launch: Launch {
                exe: Some(exe.clone()),
                working_dir: path.parent().map(|p| p.to_string_lossy().into_owned()),
                ..Default::default()
            },
            ..Default::default()
        });
        added.push(id);
    }
    added
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Candidate {
    pub exe: String,
    pub name: String,
    pub folder: String,
}

/// Programs that ship next to games but are not the game.
const NOT_GAMES: [&str; 22] = [
    "unins", "setup", "install", "redist", "vcredist", "dxsetup", "directx", "crash", "report", "helper", "updater",
    "prereq", "dotnet", "physx", "easyanticheat", "beservice", "cefsharp", "qtwebengine", "notification", "7z",
    "uplay", "touchup",
];

fn exes_in(dir: &Path, depth: usize, out: &mut Vec<(PathBuf, u64, usize)>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() && depth < 3 {
            exes_in(&path, depth + 1, out);
        } else if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe")) {
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            if !NOT_GAMES.iter().any(|n| stem.contains(n)) {
                out.push((path, e.metadata().map(|m| m.len()).unwrap_or(0), depth));
            }
        }
    }
}

/// The likeliest game program in a folder: named like the folder, then the biggest.
pub fn best_exe(folder: &Path) -> Option<PathBuf> {
    let mut found = Vec::new();
    exes_in(folder, 0, &mut found);
    let folder_key = names::normalize(&folder.file_name()?.to_string_lossy());
    found
        .into_iter()
        .max_by_key(|(path, size, depth)| {
            let stem = names::normalize(&path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default());
            let named = !stem.is_empty() && (folder_key.contains(&stem) || stem.contains(&folder_key));
            (named, *size / (1 << 20), std::cmp::Reverse(*depth))
        })
        .map(|(path, _, _)| path)
}

/// One game per sub-folder, or the folder itself when it holds a game. Skips games
/// already in the library.
pub fn scan(folder: &Path, existing: &[PathBuf]) -> Vec<Candidate> {
    let key = |p: &Path| p.to_string_lossy().to_lowercase().replace('/', "\\");
    let known: Vec<String> = existing.iter().map(|p| key(p)).filter(|k| k.len() > 3).collect();
    let taken = |p: &Path| {
        let k = key(p);
        known.iter().any(|d| k.starts_with(d.as_str()) || d.starts_with(&k))
    };
    let mut folders: Vec<PathBuf> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    if folders.is_empty() {
        folders.push(folder.to_path_buf());
    }
    let mut out: Vec<Candidate> = folders
        .into_iter()
        .filter(|f| !taken(f))
        .filter_map(|f| {
            let exe = best_exe(&f)?;
            Some(Candidate {
                name: names::from_folder(&f.to_string_lossy()).unwrap_or_else(|| game_name(&exe)),
                exe: exe.to_string_lossy().into_owned(),
                folder: f.to_string_lossy().into_owned(),
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Drops a chosen image, so the game shows its own art again.
pub fn reset_art(data: &Path, user: &mut User, id: &str, kind: Kind) {
    let Some(o) = user.overrides.get_mut(id) else { return };
    let slot = match kind {
        Kind::Cover => &mut o.art.cover,
        Kind::Background => &mut o.art.background,
        Kind::Logo => &mut o.art.logo,
    };
    if let Some(rel) = slot.take() {
        if rel.starts_with(&format!("art/{id}/custom-")) {
            let _ = fs::remove_file(data.join(rel));
        }
    }
}

/// One game found online: a search result, or the match "Change art" starts from.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub source: &'static str,
    pub id: String,
    pub name: String,
    pub platform: String,
    pub image: Option<String>,
}

/// One image to choose from.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Choice {
    pub url: String,
    /// A smaller copy for the picker.
    pub thumb: String,
    /// Where it comes from, as a person reads it: "Steam", "LaunchBox · Europe", "SteamGridDB".
    pub source: String,
}

/// Everything one match offers: its images by kind, and its description and genres.
#[derive(Serialize, Debug, Default)]
pub struct Choices {
    pub cover: Vec<Choice>,
    pub background: Vec<Choice>,
    pub logo: Vec<Choice>,
    pub description: String,
    pub genres: Vec<String>,
    /// Why SteamGridDB added nothing, when it was asked and failed.
    pub note: Option<String>,
}

impl Choices {
    fn add(&mut self, kind: Kind, choice: Choice) {
        let list = match kind {
            Kind::Cover => &mut self.cover,
            Kind::Background => &mut self.background,
            Kind::Logo => &mut self.logo,
        };
        if !list.iter().any(|c| c.url == choice.url) {
            list.push(choice);
        }
    }
}

fn launchbox_kind(kind: &str) -> Option<Kind> {
    match kind {
        "Box - Front" => Some(Kind::Cover),
        "Fanart - Background" => Some(Kind::Background),
        "Clear Logo" => Some(Kind::Logo),
        _ => None,
    }
}

/// SteamGridDB's game with the same name, if there is one.
fn steamgriddb_game(store: &Store, key: &str, name: &str) -> Option<(u64, String)> {
    let found = sgdb::Client::new(&store.agent, key).search(&names::search_name(name)).ok()?;
    found.into_iter().find(|(_, n)| names::same_game(n, &names::search_name(name)))
}

/// Where "Change art" starts: the game's Steam id when it has one, otherwise SteamGridDB's
/// game with the same name, otherwise Steam's.
pub fn first_match(store: &Store, key: Option<&str>, name: &str, steam_app: Option<u32>) -> Option<Match> {
    let steam = |id: u32, name: String| Match { source: "steam", id: id.to_string(), name, platform: "Steam".into(), image: None };
    if let Some(app) = steam_app {
        return Some(steam(app, name.to_string()));
    }
    if let Some((id, found)) = key.and_then(|k| steamgriddb_game(store, k, name)) {
        return Some(Match { source: "steamgriddb", id: id.to_string(), name: found, platform: String::new(), image: None });
    }
    let query = names::search_name(name);
    let results = store.search(&query)?;
    let app = art::best_match(&query, &results)?;
    let found = results.iter().find(|(id, _)| *id == app).map(|(_, n)| n.clone()).unwrap_or_default();
    Some(steam(app, found))
}

/// Every image a match offers: its own source's first, then SteamGridDB's for the same game.
pub fn choices(data: &Path, store: &Store, key: Option<&str>, source: &str, id: &str) -> Result<Choices, String> {
    let mut out = Choices::default();
    let grid = match source {
        "steam" => {
            let app: u32 = id.parse().map_err(|_| "Not a Steam id.")?;
            let items = store.items(&[app]);
            let item = items.get(&app).ok_or("Steam did not answer. Try again.")?;
            for kind in Kind::ALL {
                if let Some(url) = item.url(kind) {
                    out.add(kind, Choice { thumb: url.clone(), url, source: "Steam".into() });
                }
            }
            out.description = item.description.trim().to_string();
            Some(sgdb::Game::Steam(app))
        }
        "launchbox" => {
            let zip = launchbox::ensure(data, &store.agent).ok_or("The LaunchBox database could not be downloaded.")?;
            let found = launchbox::details(|each| launchbox::scan_archive(&zip, each), id).ok_or("LaunchBox has no such game.")?;
            for image in &found.images {
                if let Some(kind) = launchbox_kind(&image.kind) {
                    let source = if image.region.is_empty() { "LaunchBox".into() } else { format!("LaunchBox · {}", image.region) };
                    out.add(kind, Choice { url: image.url.clone(), thumb: image.url.clone(), source });
                }
            }
            out.description = found.overview.chars().take(600).collect::<String>().trim().to_string();
            out.genres = names::genres(&found.genres);
            key.and_then(|k| steamgriddb_game(store, k, &found.name)).map(|(id, _)| sgdb::Game::Id(id))
        }
        "steamgriddb" => Some(sgdb::Game::Id(id.parse().map_err(|_| "Not a SteamGridDB id.")?)),
        _ => return Err("Unknown source.".into()),
    };
    if let (Some(key), Some(game)) = (key, grid) {
        let client = sgdb::Client::new(&store.agent, key);
        for kind in Kind::ALL {
            match client.images(game, kind) {
                Ok(images) => {
                    for i in images {
                        out.add(kind, Choice { url: i.url, thumb: i.thumb, source: "SteamGridDB".into() });
                    }
                }
                Err(e) => {
                    out.note = Some(e);
                    break;
                }
            }
        }
    }
    Ok(out)
}

pub fn find_steamgriddb(store: &Store, key: &str, query: &str) -> Vec<Match> {
    sgdb::Client::new(&store.agent, key)
        .search(query)
        .unwrap_or_default()
        .into_iter()
        .take(8)
        .map(|(id, name)| Match { source: "steamgriddb", id: id.to_string(), name, platform: String::new(), image: None })
        .collect()
}

pub fn find_steam(store: &Store, query: &str) -> Vec<Match> {
    store
        .search(query)
        .unwrap_or_default()
        .into_iter()
        .map(|(id, name)| Match {
            source: "steam",
            image: Some(format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{id}/capsule_231x87.jpg")),
            id: id.to_string(),
            name,
            platform: "Steam".into(),
        })
        .collect()
}

pub fn find_launchbox(index: &[launchbox::Entry], query: &str) -> Vec<Match> {
    launchbox::search(index, query, 12)
        .into_iter()
        .map(|e| Match { source: "launchbox", id: e.id.clone(), name: e.name.clone(), platform: e.platform.clone(), image: None })
        .collect()
}

/// Read, change and write `user.json` in one step.
pub fn with_user<T>(data: &Path, change: impl FnOnce(&mut User) -> T) -> Result<T, String> {
    let mut user = library::read_user(data);
    let out = change(&mut user);
    library::write_user(data, &user).map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("beanbag-manage-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn edits_change_only_what_was_sent() {
        let mut user = User::default();
        apply(&mut user, "g", Patch { name: Some(" Hades ".into()), ..Default::default() });
        apply(&mut user, "g", Patch { catalogs: Some(vec!["Couch".into(), "couch".into(), " ".into()]), ..Default::default() });
        let o = &user.overrides["g"];
        assert_eq!(o.name.as_deref(), Some("Hades"));
        assert_eq!(o.catalogs.as_deref(), Some(&["Couch".to_string()][..]));
        assert_eq!(o.description, None);
    }

    #[test]
    fn finds_the_folder_that_names_the_game() {
        assert_eq!(install_root(Path::new("/g/Hades/x64/Hades.exe")), Path::new("/g/Hades"));
        assert_eq!(install_root(Path::new("/g/Dead Static Drive/Binaries/Win64/x.exe")), Path::new("/g/Dead Static Drive"));
        assert_eq!(install_root(Path::new("/g/Celeste/Celeste.exe")), Path::new("/g/Celeste"));
    }

    #[test]
    fn adds_new_programs_once() {
        let mut user = User::default();
        let first = add(&mut user, &[], &["/g/Hades/Hades.exe".into(), "/g/Hades/Hades.exe".into()]);
        assert_eq!(first.len(), 1);
        assert_eq!(user.games[0].name, "Hades");
        assert_eq!(user.games[0].install_dir, "/g/Hades");
        assert!(add(&mut user, &[], &["/g/Hades/Hades.exe".into()]).is_empty());
        assert!(add(&mut user, &[PathBuf::from("/g/Celeste/Celeste.exe")], &["/G/celeste/celeste.exe".into()]).is_empty());
    }

    #[test]
    fn scans_one_game_per_folder_and_skips_tools_and_known_games() {
        let root = temp("scan");
        let mk = |p: &str, bytes: usize| {
            let path = root.join(p);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, vec![0u8; bytes]).unwrap();
        };
        mk("Hollow Knight/hollow_knight.exe", 10);
        mk("Hollow Knight/UnityCrashHandler64.exe", 2_000_000);
        mk("Celeste.v1.4-GOG/Celeste.exe", 10);
        mk("Celeste.v1.4-GOG/unins000.exe", 10);
        mk("Known Game/game.exe", 10);
        mk("Docs/readme.txt", 10);
        let found = scan(&root, &[root.join("Known Game")]);
        let names: Vec<_> = found.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Celeste", "Hollow Knight"]);
        assert!(found[1].exe.ends_with("hollow_knight.exe"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn custom_art_replaces_the_last_custom_art() {
        let data = temp("art");
        let src = data.join("pick.png");
        fs::write(&src, b"png").unwrap();
        let mut user = User::default();
        set_art(&data, &mut user, "g", Kind::Logo, &src.to_string_lossy()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        set_art(&data, &mut user, "g", Kind::Logo, &src.to_string_lossy()).unwrap();
        let files: Vec<_> = fs::read_dir(data.join("art/g")).unwrap().flatten().collect();
        assert_eq!(files.len(), 1);
        assert!(user.overrides["g"].art.logo.as_deref().unwrap().starts_with("art/g/custom-logo-"));
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn original_art_drops_the_chosen_image_and_its_file() {
        let data = temp("reset");
        let src = data.join("pick.png");
        fs::write(&src, b"png").unwrap();
        let mut user = User::default();
        set_art(&data, &mut user, "g", Kind::Logo, &src.to_string_lossy()).unwrap();
        set_art(&data, &mut user, "g", Kind::Cover, &src.to_string_lossy()).unwrap();
        reset_art(&data, &mut user, "g", Kind::Logo);
        assert_eq!(user.overrides["g"].art.logo, None);
        assert!(user.overrides["g"].art.cover.is_some(), "only the one kind goes");
        let left: Vec<_> = fs::read_dir(data.join("art/g")).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left.len(), 1);
        reset_art(&data, &mut user, "unknown", Kind::Logo);
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn lists_each_image_once_under_its_kind() {
        let mut choices = Choices::default();
        let pick = |url: &str, source: &str| Choice { url: url.into(), thumb: url.into(), source: source.into() };
        choices.add(Kind::Logo, pick("https://a/logo.png", "Steam"));
        choices.add(Kind::Logo, pick("https://a/logo.png", "SteamGridDB"));
        choices.add(Kind::Cover, pick("https://a/cover.jpg", "Steam"));
        assert_eq!(choices.logo.len(), 1);
        assert_eq!(choices.logo[0].source, "Steam");
        assert_eq!(choices.cover.len(), 1);
        assert_eq!(launchbox_kind("Clear Logo"), Some(Kind::Logo));
        assert_eq!(launchbox_kind("Screenshot - Gameplay"), None);
    }

    #[test]
    fn removing_an_imported_game_hides_it() {
        let data = temp("remove");
        let mut user = User::default();
        add(&mut user, &[], &["/g/Mine/mine.exe".into()]);
        let mine = user.games[0].id.clone();
        remove(&data, &mut user, &mine);
        assert!(user.games.is_empty());
        remove(&data, &mut user, "imported");
        assert!(user.overrides["imported"].hidden);
        fs::remove_dir_all(data).unwrap();
    }
}
