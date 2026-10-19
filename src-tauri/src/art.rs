//! Fills missing covers, backgrounds, logos, descriptions and genres. Steam first (official
//! art), then the LaunchBox database for everything Steam does not sell. Runs in the
//! background after start-up and never replaces anything that is already there.

use crate::{launchbox, names};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

const FIXES_FILE: &str = "fixes.json";
const ASSETS: &str = "https://shared.akamai.steamstatic.com/store_item_assets/";
/// A game no source knew is asked about again after a week.
const RETRY_AFTER: i64 = 7 * 86_400;
/// Bumped when the fix learns something new to look for (2: LaunchBox and genres;
/// 3: Steam tags and player modes for the rows; 4: popularity and release dates),
/// so older checks run again.
pub const PASS: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Cover,
    Background,
    Logo,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Cover, Kind::Background, Kind::Logo];

    pub fn stem(self) -> &'static str {
        match self {
            Kind::Cover => "cover",
            Kind::Background => "background",
            Kind::Logo => "logo",
        }
    }
}

/// One game with something missing.
#[derive(Debug, Clone, Default)]
pub struct Need {
    pub id: String,
    pub name: String,
    pub install_dir: String,
    pub platform: String,
    pub app_id: Option<u32>,
    pub kinds: Vec<Kind>,
    pub description: bool,
    pub genres: bool,
    /// How the game is played (tags, player modes) is still unknown.
    pub traits: bool,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Fix {
    #[serde(default)]
    pub app_id: Option<u32>,
    #[serde(default)]
    pub checked: i64,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub pass: u32,
    /// Steam tags, strongest first.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Steam supported-player categories.
    #[serde(default)]
    pub players: Vec<u32>,
    /// Steam feature categories.
    #[serde(default)]
    pub features: Vec<u32>,
    /// From LaunchBox, for games Steam does not sell.
    #[serde(default)]
    pub max_players: u32,
    /// Steam review count, or LaunchBox rating count: how many people play it.
    #[serde(default)]
    pub popularity: u64,
    /// Unix seconds; 0 when unknown.
    #[serde(default)]
    pub released: i64,
}

impl Fix {
    /// The current pass has looked up how the game is played, its popularity and its date.
    pub fn knows_traits(&self) -> bool {
        self.pass >= PASS && (!self.tags.is_empty() || !self.players.is_empty() || self.max_players > 0)
    }
}

pub fn read_fixes(data: &Path) -> HashMap<String, Fix> {
    fs::read_to_string(data.join(FIXES_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn write_fixes(data: &Path, fixes: &HashMap<String, Fix>) {
    let tmp = data.join(format!("{FIXES_FILE}.tmp"));
    if serde_json::to_vec(fixes).ok().and_then(|b| fs::write(&tmp, b).ok()).is_some() {
        let _ = fs::rename(tmp, data.join(FIXES_FILE));
    }
}

/// Art a fix downloaded: `art/<id>/<kind>.<ext>` under the data folder.
pub fn on_disk(data: &Path, id: &str, kind: Kind) -> Option<PathBuf> {
    ["jpg", "png", "webp", "jpeg"]
        .iter()
        .map(|ext| data.join("art").join(id).join(format!("{}.{ext}", kind.stem())))
        .find(|p| p.is_file())
}

/// The best Steam result for one of a game's names: the same name first, then an edition.
pub fn best_match(name: &str, results: &[(u32, String)]) -> Option<u32> {
    let want = names::normalize(name);
    if want.is_empty() {
        return None;
    }
    results
        .iter()
        .find(|(_, n)| names::normalize(n) == want)
        .or_else(|| results.iter().find(|(_, n)| names::same_game(n, name)))
        .map(|(id, _)| *id)
}

pub struct Store {
    pub agent: ureq::Agent,
}

#[derive(Default)]
pub struct Item {
    pub url_format: String,
    pub cover: Option<String>,
    pub background: Option<String>,
    pub description: String,
    pub tags: Vec<u32>,
    pub players: Vec<u32>,
    pub features: Vec<u32>,
    pub popularity: u64,
    pub released: i64,
}

impl Item {
    /// The image URL for one kind of art.
    pub fn url(&self, kind: Kind) -> Option<String> {
        let file = match kind {
            Kind::Cover => self.cover.clone().unwrap_or_else(|| "library_600x900_2x.jpg".into()),
            Kind::Background => self.background.clone().unwrap_or_else(|| "library_hero.jpg".into()),
            Kind::Logo => "logo.png".into(),
        };
        asset_url(&self.url_format, &file)
    }
}

impl Store {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(120)))
            .user_agent("Beanbag")
            .build()
            .into();
        Store { agent }
    }

    fn json(&self, url: &str, query: &[(&str, &str)]) -> Option<serde_json::Value> {
        let mut req = self.agent.get(url);
        for (k, v) in query {
            req = req.query(*k, *v);
        }
        req.call().ok()?.body_mut().read_json().ok()
    }

    /// None when the request failed, so the game is asked about again next time.
    pub fn search(&self, name: &str) -> Option<Vec<(u32, String)>> {
        let v = self.json("https://store.steampowered.com/api/storesearch/", &[("term", name), ("cc", "us"), ("l", "en")])?;
        Some(
            v["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|i| Some((i["id"].as_u64()? as u32, i["name"].as_str()?.to_string())))
                .collect(),
        )
    }

    pub fn items(&self, ids: &[u32]) -> HashMap<u32, Item> {
        let mut out = HashMap::new();
        for chunk in ids.chunks(40) {
            let input = serde_json::json!({
                "ids": chunk.iter().map(|id| serde_json::json!({ "appid": id })).collect::<Vec<_>>(),
                "context": { "language": "english", "country_code": "US" },
                "data_request": {
                    "include_assets": true,
                    "include_basic_info": true,
                    "include_tag_count": 20,
                    "include_reviews": true,
                    "include_release": true
                },
            })
            .to_string();
            let Some(v) = self.json("https://api.steampowered.com/IStoreBrowseService/GetItems/v1/", &[("input_json", &input)])
            else {
                continue;
            };
            for item in v["response"]["store_items"].as_array().into_iter().flatten() {
                let Some(id) = item["appid"].as_u64() else { continue };
                let a = &item["assets"];
                let pick = |keys: &[&str]| keys.iter().find_map(|k| a[*k].as_str().map(String::from));
                let ids = |v: &serde_json::Value| -> Vec<u32> {
                    v.as_array().into_iter().flatten().filter_map(|x| x.as_u64().map(|n| n as u32)).collect()
                };
                let cats = &item["categories"];
                out.insert(
                    id as u32,
                    Item {
                        url_format: a["asset_url_format"].as_str().unwrap_or_default().to_string(),
                        cover: pick(&["library_capsule_2x", "library_capsule"]),
                        background: pick(&["library_hero"]),
                        description: item["basic_info"]["short_description"].as_str().unwrap_or_default().to_string(),
                        // Tags come strongest first.
                        tags: item["tags"].as_array().into_iter().flatten().filter_map(|t| t["tagid"].as_u64().map(|n| n as u32)).collect(),
                        players: ids(&cats["supported_player_categoryids"]),
                        features: ids(&cats["feature_categoryids"]),
                        popularity: item["reviews"]["summary_filtered"]["review_count"].as_u64().unwrap_or(0),
                        released: ["original_release_date", "steam_release_date"]
                            .iter()
                            .find_map(|k| item["release"][*k].as_i64().filter(|t| *t > 0))
                            .unwrap_or(0),
                    },
                );
            }
            thread::sleep(Duration::from_millis(300));
        }
        out
    }

    /// Steam's tag names by id.
    pub fn tag_names(&self) -> HashMap<u32, String> {
        let Some(v) = self.json("https://api.steampowered.com/IStoreService/GetTagList/v1/", &[("language", "english")]) else {
            return HashMap::new();
        };
        v["response"]["tags"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|t| Some((t["tagid"].as_u64()? as u32, t["name"].as_str()?.to_string())))
            .collect()
    }

    /// Saves an image if the source has it. False for 404s and anything that is not an image.
    pub fn download(&self, url: &str, dest: &Path) -> bool {
        let Ok(mut resp) = self.agent.get(url).call() else { return false };
        let is_image = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|t| t.starts_with("image/"));
        let Ok(bytes) = resp.body_mut().with_config().limit(25 * 1024 * 1024).read_to_vec() else { return false };
        if !is_image || bytes.len() < 1024 {
            return false;
        }
        let tmp = dest.with_extension("part");
        fs::create_dir_all(dest.parent().unwrap_or(Path::new("."))).is_ok()
            && fs::write(&tmp, bytes).is_ok()
            && fs::rename(&tmp, dest).is_ok()
    }
}

fn asset_url(format: &str, file: &str) -> Option<String> {
    (!format.is_empty()).then(|| format!("{ASSETS}{}", format.replace("${FILENAME}", file)))
}

pub fn extension(file: &str) -> &str {
    let file = file.split('?').next().unwrap_or(file);
    match file.rsplit('.').next() {
        Some("png") => "png",
        Some("webp") => "webp",
        _ => "jpg",
    }
}

fn dest(data: &Path, id: &str, kind: Kind, file: &str) -> PathBuf {
    data.join("art").join(id).join(format!("{}.{}", kind.stem(), extension(file)))
}

/// Steam: official art and a short description, for games Steam sells.
/// Returns the games whose search failed outright, to retry at the next start.
fn from_steam(store: &Store, data: &Path, needs: &[Need], fixes: &mut HashMap<String, Fix>) -> Vec<String> {
    let mut ids: HashMap<String, u32> = HashMap::new();
    let mut failed = Vec::new();
    for n in needs {
        let known = fixes.get(&n.id);
        let only_traits = n.kinds.is_empty() && !n.description && !n.genres;
        let id = match n.app_id.or(known.and_then(|f| f.app_id)) {
            Some(id) => Some(id),
            // Searched before with no match, and only traits are wanted: LaunchBox covers it.
            None if only_traits && known.is_some_and(|f| f.checked > 0) => None,
            None => {
                let mut found = None;
                for name in names::variants(&n.name, &n.install_dir).iter().take(3) {
                    let results = store.search(name);
                    thread::sleep(Duration::from_millis(250));
                    match results {
                        Some(results) => {
                            found = best_match(name, &results);
                            if found.is_some() {
                                break;
                            }
                        }
                        None => {
                            failed.push(n.id.clone());
                            break;
                        }
                    }
                }
                found
            }
        };
        if let Some(id) = id {
            ids.insert(n.id.clone(), id);
        }
    }
    let mut wanted: Vec<u32> = ids.values().copied().collect();
    wanted.sort_unstable();
    wanted.dedup();
    let items = store.items(&wanted);
    let tag_names = if needs.iter().any(|n| n.traits) { store.tag_names() } else { HashMap::new() };

    for n in needs {
        let Some(&app_id) = ids.get(&n.id) else { continue };
        let Some(item) = items.get(&app_id) else { continue };
        for &kind in &n.kinds {
            if let Some(url) = item.url(kind) {
                store.download(&url, &dest(data, &n.id, kind, &url));
            }
        }
        let fix = fixes.entry(n.id.clone()).or_default();
        fix.app_id = Some(app_id);
        if n.description && fix.description.is_empty() {
            fix.description = item.description.trim().to_string();
        }
        if n.traits && !tag_names.is_empty() {
            fix.tags = item.tags.iter().filter_map(|t| tag_names.get(t).cloned()).collect();
            fix.players = item.players.clone();
            fix.features = item.features.clone();
            fix.popularity = item.popularity;
            fix.released = item.released;
        }
    }
    failed
}

/// LaunchBox: every platform, for what Steam left missing.
fn from_launchbox(store: &Store, data: &Path, needs: &[Need], fixes: &mut HashMap<String, Fix>) {
    let left: Vec<&Need> = needs
        .iter()
        .filter(|n| {
            let fix = fixes.get(&n.id);
            n.kinds.iter().any(|&k| on_disk(data, &n.id, k).is_none())
                || (n.description && fix.is_none_or(|f| f.description.is_empty()))
                || (n.genres && fix.is_none_or(|f| f.genres.is_empty()))
                // Steam gave tags, popularity and dates for its own games; LaunchBox does the rest.
                || (n.traits && fix.is_none_or(|f| f.app_id.is_none()))
        })
        .collect();
    if left.is_empty() {
        return;
    }
    let Some(zip) = launchbox::ensure(data, &store.agent) else { return };
    let wants: Vec<launchbox::Want> = left
        .iter()
        .map(|n| launchbox::Want {
            id: n.id.clone(),
            names: names::variants(&n.name, &n.install_dir),
            platform: n.platform.clone(),
        })
        .collect();
    let found = launchbox::lookup(|each| launchbox::scan_archive(&zip, each), &wants);

    for n in left {
        let Some(game) = found.get(&n.id) else { continue };
        for &kind in &n.kinds {
            if on_disk(data, &n.id, kind).is_some() {
                continue;
            }
            let url = match kind {
                Kind::Cover => &game.cover,
                Kind::Background => &game.background,
                Kind::Logo => &game.logo,
            };
            if let Some(url) = url {
                store.download(url, &dest(data, &n.id, kind, url));
            }
        }
        let fix = fixes.entry(n.id.clone()).or_default();
        if n.description && fix.description.is_empty() {
            fix.description = game.overview.chars().take(600).collect();
        }
        if n.genres && fix.genres.is_empty() {
            fix.genres = game.genres.clone();
        }
        if n.traits && fix.app_id.is_none() {
            fix.max_players = game.max_players;
            fix.popularity = game.rating_count;
            fix.released = game.released;
        }
    }
}

/// Fixes what it can and returns how many things changed.
pub fn fix(data: &Path, needs: Vec<Need>, now: i64) -> usize {
    let mut fixes = read_fixes(data);
    let needs: Vec<Need> = needs
        .into_iter()
        .filter(|n| fixes.get(&n.id).is_none_or(|f| f.pass < PASS || now - f.checked > RETRY_AFTER))
        .collect();
    if needs.is_empty() {
        return 0;
    }
    let art = |needs: &[Need]| -> usize {
        needs.iter().map(|n| n.kinds.iter().filter(|&&k| on_disk(data, &n.id, k).is_some()).count()).sum()
    };
    let before = fixes.clone();
    let art_before = art(&needs);
    let store = Store::new();

    let failed = from_steam(&store, data, &needs, &mut fixes);
    from_launchbox(&store, data, &needs, &mut fixes);

    // A search that failed outright is retried next start; everything else waits a week.
    for n in &needs {
        if !failed.contains(&n.id) {
            let fix = fixes.entry(n.id.clone()).or_default();
            fix.checked = now;
            fix.pass = PASS;
        }
    }
    write_fixes(data, &fixes);

    let text = needs
        .iter()
        .filter(|n| {
            let (old, new) = (before.get(&n.id), fixes.get(&n.id));
            let key = |f: &Fix| (f.description.clone(), f.genres.clone(), f.tags.clone(), f.max_players, f.popularity, f.released);
            old.map(key) != new.map(key)
        })
        .count();
    art(&needs) - art_before + text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_results_match_by_name_then_edition() {
        let results = [(2215430, "Ghost of Tsushima DIRECTOR'S CUT".to_string()), (1, "Ghost Recon".into())];
        assert_eq!(best_match("Ghost of Tsushima: Director's Cut", &results), Some(2215430));
        let horizon = [(2561580, "Horizon Zero Dawn™ Remastered".to_string()), (804410, "The Making of Horizon Zero Dawn".into())];
        assert_eq!(best_match("Horizon Zero Dawn", &horizon), Some(2561580));
        assert_eq!(best_match("Portal", &[(1, "Portal 2".into()), (2, "Portal Knights".into())]), None);
        assert_eq!(best_match("", &[(1, "x".into())]), None);
    }

    #[test]
    fn builds_asset_urls_and_keeps_the_extension() {
        let url = asset_url("steam/apps/620/abc/${FILENAME}?t=1", "logo.png").unwrap();
        assert_eq!(url, "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/620/abc/logo.png?t=1");
        assert_eq!(extension("library_600x900_2x.jpg?t=3"), "jpg");
        assert_eq!(extension("https://images.launchbox-app.com/c.png"), "png");
        assert!(asset_url("", "logo.png").is_none());
    }

    #[test]
    fn finds_downloaded_art_by_kind() {
        let data = std::env::temp_dir().join(format!("beanbag-art-fix-{}", std::process::id()));
        fs::create_dir_all(data.join("art").join("g")).unwrap();
        fs::write(data.join("art").join("g").join("logo.png"), b"x").unwrap();
        assert!(on_disk(&data, "g", Kind::Logo).is_some());
        assert!(on_disk(&data, "g", Kind::Cover).is_none());
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn games_checked_recently_are_skipped_without_network() {
        let data = std::env::temp_dir().join(format!("beanbag-fixes-{}", std::process::id()));
        fs::create_dir_all(&data).unwrap();
        let mut fixes = HashMap::new();
        fixes.insert("g".to_string(), Fix { checked: 1_000, pass: PASS, ..Default::default() });
        // Checked by an older pass: how it is played is not known yet.
        assert!(!Fix { tags: vec!["Indie".into()], pass: PASS - 1, ..Default::default() }.knows_traits());
        assert!(Fix { tags: vec!["Indie".into()], pass: PASS, ..Default::default() }.knows_traits());
        write_fixes(&data, &fixes);
        let need = Need { id: "g".into(), name: "x".into(), kinds: vec![Kind::Cover], description: true, ..Default::default() };
        assert_eq!(fix(&data, vec![need], 1_000 + 60), 0);
        fs::remove_dir_all(data).unwrap();
    }
}
