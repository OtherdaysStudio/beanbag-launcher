//! The LaunchBox games database: a public archive of about 190,000 games on every
//! platform, with box art, logos, backgrounds, descriptions and genres. No key needed.
//! The archive is read as a stream, straight out of the zip.

use crate::names;
use quick_xml::{events::Event, Reader};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

const URL: &str = "https://gamesdb.launchbox-app.com/Metadata.zip";
pub const IMAGES: &str = "https://images.launchbox-app.com/";
const MAX_AGE: Duration = Duration::from_secs(30 * 86_400);

/// One game to find, by its names in order of preference.
pub struct Want {
    pub id: String,
    pub names: Vec<String>,
    pub platform: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Found {
    pub name: String,
    pub platform: String,
    pub overview: String,
    pub genres: Vec<String>,
    pub max_players: u32,
    pub rating_count: u64,
    /// Unix seconds; 0 when unknown.
    pub released: i64,
    pub cover: Option<String>,
    pub logo: Option<String>,
    pub background: Option<String>,
    /// Every image of the game, best region first; only `details` fills it.
    pub images: Vec<Image>,
}

/// One image in the database.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Image {
    /// LaunchBox's own type, like "Box - Front" or "Clear Logo".
    pub kind: String,
    pub url: String,
    pub region: String,
}

/// The archive on disk, downloaded when missing or a month old.
pub fn ensure(data: &Path, agent: &ureq::Agent) -> Option<PathBuf> {
    let path = data.join("launchbox").join("Metadata.zip");
    let fresh = fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < MAX_AGE);
    if fresh {
        return Some(path);
    }
    fs::create_dir_all(path.parent()?).ok()?;
    let mut resp = agent.get(URL).call().ok()?;
    let tmp = path.with_extension("part");
    let mut file = fs::File::create(&tmp).ok()?;
    std::io::copy(&mut resp.body_mut().as_reader(), &mut file).ok()?;
    drop(file);
    fs::rename(&tmp, &path).ok()?;
    Some(path)
}

pub type Fields = HashMap<&'static str, String>;

const FIELDS: [&str; 12] = [
    "Name",
    "DatabaseID",
    "Platform",
    "Overview",
    "Genres",
    "MaxPlayers",
    "CommunityRatingCount",
    "ReleaseDate",
    "AlternateName",
    "FileName",
    "Type",
    "Region",
];

/// "2017-03-03T00:00:00-08:00" or "2017-03-03" → Unix seconds at midnight UTC.
pub fn date(text: &str) -> i64 {
    let mut parts = text.get(..10).unwrap_or("").split('-').map(|p| p.parse::<i64>().ok());
    let (Some(Some(y)), Some(Some(m)), Some(Some(d))) = (parts.next(), parts.next(), parts.next()) else { return 0 };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return 0;
    }
    // Days from the civil calendar (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) * 86_400
}

/// Calls `each` with every top-level record (Game, GameAlternateName, GameImage, …)
/// and the text of the fields above.
pub fn records(xml: impl Read, mut each: impl FnMut(&str, &Fields)) {
    let mut reader = Reader::from_reader(BufReader::with_capacity(1 << 16, xml));
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut record = String::new();
    let mut field: Option<&'static str> = None;
    let mut fields: HashMap<&'static str, String> = HashMap::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                let name = e.name().0;
                if depth == 2 {
                    record = name.to_string();
                    fields.clear();
                } else if depth == 3 {
                    field = FIELDS.iter().find(|f| **f == name).copied();
                }
            }
            Ok(Event::Text(t)) => {
                if let Some(f) = field {
                    fields.entry(f).or_default().push_str(&t.xml10_content());
                }
            }
            Ok(Event::GeneralRef(r)) => {
                let c = match r.resolve_char_ref() {
                    Ok(Some(c)) => Some(c),
                    _ => match &*r {
                        "amp" => Some('&'),
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "quot" => Some('"'),
                        "apos" => Some('\''),
                        _ => None,
                    },
                };
                if let (Some(f), Some(c)) = (field, c) {
                    fields.entry(f).or_default().push(c);
                }
            }
            Ok(Event::End(_)) => {
                match depth {
                    3 => field = None,
                    2 => each(&record, &fields),
                    _ => {}
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

/// Platforms to prefer when the game's own platform is unknown: PC, then current consoles.
const PREFERRED: [&str; 10] = [
    "windows",
    "nintendoswitch",
    "nintendoswitch2",
    "sonyplaystation5",
    "sonyplaystation4",
    "microsoftxboxseriesxs",
    "microsoftxboxone",
    "nintendowiiu",
    "sonyplaystation3",
    "microsoftxbox360",
];

pub fn platform_rank(platform: &str, wanted: &str) -> usize {
    let (p, w) = (names::normalize(platform), names::normalize(wanted));
    if !w.is_empty() && !p.is_empty() && (w.contains(&p) || p.contains(&w)) {
        return 0;
    }
    PREFERRED.iter().position(|x| *x == p).map_or(50, |i| i + 1)
}

const REGIONS: [&str; 6] = ["", "world", "northamerica", "unitedstates", "europe", "unitedkingdom"];

fn region_rank(region: &str) -> usize {
    REGIONS.iter().position(|r| *r == names::normalize(region)).unwrap_or(10)
}

fn image(images: &[(String, String, String)], kind: &str) -> Option<String> {
    images
        .iter()
        .filter(|(t, _, _)| t == kind)
        .min_by_key(|(_, _, region)| region_rank(region))
        .map(|(_, file, _)| format!("{IMAGES}{file}"))
}

fn info(f: &Fields) -> Found {
    let get = |k: &str| f.get(k).map(|s| s.trim().to_string()).unwrap_or_default();
    Found {
        name: get("Name"),
        platform: get("Platform"),
        overview: get("Overview"),
        genres: get("Genres").split(';').map(str::trim).filter(|g| !g.is_empty()).map(String::from).collect(),
        max_players: get("MaxPlayers").parse().unwrap_or(0),
        rating_count: get("CommunityRatingCount").parse().unwrap_or(0),
        released: date(&get("ReleaseDate")),
        ..Default::default()
    }
}

/// Finds each wanted game. `scan` makes one pass over the database (see `scan_archive`).
pub fn lookup(scan: impl Fn(&mut dyn FnMut(&str, &Fields)) -> bool, wants: &[Want]) -> HashMap<String, Found> {
    let mut index: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    for (w, want) in wants.iter().enumerate() {
        for (rank, name) in want.names.iter().enumerate() {
            index.entry(names::normalize(name)).or_default().push((w, rank));
        }
    }
    // (want, name rank, database id)
    let mut hits: Vec<(usize, usize, String)> = Vec::new();
    let mut games: HashMap<String, Found> = HashMap::new();
    let mut images: HashMap<String, Vec<(String, String, String)>> = HashMap::new();
    let mut ids: HashSet<String> = HashSet::new();
    let field = |f: &Fields, k: &str| f.get(k).map(|s| s.trim().to_string()).unwrap_or_default();

    // Games come first in the file, then other names, then images.
    let read = scan(&mut |tag, f| match tag {
        "Game" | "GameAlternateName" => {
            let name = field(f, if tag == "Game" { "Name" } else { "AlternateName" });
            let Some(found) = index.get(&names::normalize(&name)) else { return };
            let id = field(f, "DatabaseID");
            for &(w, rank) in found {
                hits.push((w, rank, id.clone()));
            }
            if tag == "Game" {
                games.insert(id.clone(), info(f));
            }
            ids.insert(id);
        }
        "GameImage" => {
            let id = field(f, "DatabaseID");
            if ids.contains(&id) {
                images.entry(id).or_default().push((field(f, "Type"), field(f, "FileName"), field(f, "Region")));
            }
        }
        _ => {}
    });
    if !read {
        return HashMap::new();
    }

    // A game found only by another name needs a second pass for its details.
    let missing: HashSet<String> = ids.iter().filter(|id| !games.contains_key(*id)).cloned().collect();
    if !missing.is_empty() {
        scan(&mut |tag, f| {
            if tag == "Game" {
                let id = field(f, "DatabaseID");
                if missing.contains(&id) {
                    games.insert(id, info(f));
                }
            }
        });
    }

    let mut out = HashMap::new();
    for (w, want) in wants.iter().enumerate() {
        let empty = Vec::new();
        let best = hits
            .iter()
            .filter(|(hw, _, id)| *hw == w && games.contains_key(id))
            .min_by_key(|(_, rank, id)| {
                let imgs = images.get(id).unwrap_or(&empty);
                let richness = ["Box - Front", "Clear Logo", "Fanart - Background"]
                    .iter()
                    .filter(|k| imgs.iter().any(|(t, _, _)| t == *k))
                    .count();
                (*rank, platform_rank(&games[id].platform, &want.platform), 3 - richness)
            });
        if let Some((_, _, id)) = best {
            let imgs = images.get(id).unwrap_or(&empty);
            let mut found = games[id].clone();
            found.cover = image(imgs, "Box - Front");
            found.logo = image(imgs, "Clear Logo");
            found.background = image(imgs, "Fanart - Background");
            out.insert(want.id.clone(), found);
        }
    }
    out
}

/// One game in the name index used by "Find online".
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub platform: String,
    key: String,
}

/// Every game's name and platform, for searching by hand (about 190,000 rows).
pub fn index(scan: impl Fn(&mut dyn FnMut(&str, &Fields)) -> bool) -> Vec<Entry> {
    let mut out = Vec::new();
    scan(&mut |tag, f| {
        if tag == "Game" {
            let get = |k: &str| f.get(k).map(|s| s.trim().to_string()).unwrap_or_default();
            let name = get("Name");
            out.push(Entry { key: names::normalize(&name), id: get("DatabaseID"), platform: get("Platform"), name });
        }
    });
    out
}

/// Best matches for what someone typed: exact names, then names that start with it, then the rest.
pub fn search<'a>(index: &'a [Entry], query: &str, limit: usize) -> Vec<&'a Entry> {
    let q = names::normalize(query);
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(u8, usize, &Entry)> = index
        .iter()
        .filter_map(|e| {
            let rank = if e.key == q {
                0
            } else if e.key.starts_with(&q) {
                1
            } else if e.key.contains(&q) {
                2
            } else {
                return None;
            };
            Some((rank, platform_rank(&e.platform, ""), e))
        })
        .collect();
    hits.sort_by_key(|(rank, platform, e)| (*rank, *platform, e.key.len()));
    hits.into_iter().take(limit).map(|(_, _, e)| e).collect()
}

/// Full details and best images for one game by its database id.
pub fn details(scan: impl Fn(&mut dyn FnMut(&str, &Fields)) -> bool, id: &str) -> Option<Found> {
    let mut found: Option<Found> = None;
    let mut images = Vec::new();
    scan(&mut |tag, f| {
        let get = |k: &str| f.get(k).map(|s| s.trim().to_string()).unwrap_or_default();
        if get("DatabaseID") != id {
            return;
        }
        match tag {
            "Game" => found = Some(info(f)),
            "GameImage" => images.push((get("Type"), get("FileName"), get("Region"))),
            _ => {}
        }
    });
    let mut game = found?;
    game.cover = image(&images, "Box - Front");
    game.logo = image(&images, "Clear Logo");
    game.background = image(&images, "Fanart - Background");
    images.sort_by_key(|(_, _, region)| region_rank(region));
    game.images = images
        .into_iter()
        .map(|(kind, file, region)| Image { kind, url: format!("{IMAGES}{file}"), region })
        .collect();
    Some(game)
}

/// One pass over the XML inside the archive on disk. False when it cannot be read.
pub fn scan_archive(zip: &Path, each: &mut dyn FnMut(&str, &Fields)) -> bool {
    let Ok(file) = fs::File::open(zip) else { return false };
    let Ok(mut archive) = zip::ZipArchive::new(file) else { return false };
    let Ok(entry) = archive.by_name("Metadata.xml") else { return false };
    records(entry, each);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" standalone="yes"?>
<LaunchBox>
  <Game><Name>The Legend of Zelda</Name><DatabaseID>1</DatabaseID><Platform>Nintendo Entertainment System</Platform></Game>
  <Game><Name>The Legend of Zelda: Tears of the Kingdom</Name><DatabaseID>2</DatabaseID><Platform>Nintendo Switch</Platform><MaxPlayers>1</MaxPlayers><ReleaseDate>2023-05-12T00:00:00-07:00</ReleaseDate><CommunityRatingCount>42</CommunityRatingCount>
    <Overview>Sky &amp; land &#8212; again.</Overview><Genres>Action; Adventure</Genres></Game>
  <Game><Name>Alan Wake II</Name><DatabaseID>3</DatabaseID><Platform>Sony Playstation 5</Platform></Game>
  <Game><Name>Alan Wake II</Name><DatabaseID>4</DatabaseID><Platform>Windows</Platform></Game>
  <Game><Name>Pocket Monsters Sword</Name><DatabaseID>5</DatabaseID><Platform>Nintendo Switch</Platform></Game>
  <GameAlternateName><AlternateName>Pokémon Sword</AlternateName><DatabaseID>5</DatabaseID></GameAlternateName>
  <GameImage><DatabaseID>2</DatabaseID><FileName>a.jpg</FileName><Type>Box - Front</Type><Region>Japan</Region></GameImage>
  <GameImage><DatabaseID>2</DatabaseID><FileName>b.jpg</FileName><Type>Box - Front</Type><Region>North America</Region></GameImage>
  <GameImage><DatabaseID>2</DatabaseID><FileName>c.png</FileName><Type>Clear Logo</Type></GameImage>
  <GameImage><DatabaseID>4</DatabaseID><FileName>d.jpg</FileName><Type>Fanart - Background</Type></GameImage>
</LaunchBox>"#;

    fn scan_xml(each: &mut dyn FnMut(&str, &Fields)) -> bool {
        records(XML.as_bytes(), each);
        true
    }

    fn want(id: &str, names: &[&str], platform: &str) -> Want {
        Want { id: id.into(), names: names.iter().map(|s| s.to_string()).collect(), platform: platform.into() }
    }

    #[test]
    fn finds_by_full_name_with_details_and_best_region_art() {
        let wants = [want("totk", &["The Legend of Zelda: Tears of the Kingdom"], "")];
        let found = lookup(scan_xml, &wants);
        let totk = &found["totk"];
        assert_eq!(totk.platform, "Nintendo Switch");
        assert_eq!(totk.overview, "Sky & land — again.");
        assert_eq!(totk.genres, ["Action", "Adventure"]);
        assert_eq!(totk.max_players, 1);
        assert_eq!(totk.rating_count, 42);
        assert_eq!(totk.released, 1_683_849_600);
        assert_eq!(totk.cover.as_deref(), Some("https://images.launchbox-app.com/b.jpg"));
        assert_eq!(totk.logo.as_deref(), Some("https://images.launchbox-app.com/c.png"));
        assert_eq!(totk.background, None);
    }

    #[test]
    fn prefers_the_games_platform_then_windows() {
        let pc = lookup(scan_xml, &[want("a", &["Alan Wake II"], "")]);
        assert_eq!(pc["a"].platform, "Windows");
        let ps5 = lookup(scan_xml, &[want("a", &["Alan Wake II"], "Sony PlayStation 5")]);
        assert_eq!(ps5["a"].platform, "Sony Playstation 5");
    }

    #[test]
    fn finds_by_another_name() {
        let found = lookup(scan_xml, &[want("p", &["Pokémon Sword & Shield Double Pack", "Pokémon Sword"], "")]);
        assert_eq!(found["p"].name, "Pocket Monsters Sword");
    }

    #[test]
    fn searches_names_by_hand_and_reads_one_game() {
        let index = index(scan_xml);
        assert_eq!(index.len(), 5);
        let hits: Vec<_> = search(&index, "alan wake", 5).iter().map(|e| e.platform.clone()).collect();
        assert_eq!(hits, ["Windows", "Sony Playstation 5"]);
        assert!(search(&index, "  ", 5).is_empty());
        let totk = details(scan_xml, "2").unwrap();
        assert_eq!(totk.genres, ["Action", "Adventure"]);
        assert!(totk.logo.is_some());
        assert!(details(scan_xml, "999").is_none());
    }

    #[test]
    fn lists_every_image_of_one_game_best_region_first() {
        let totk = details(scan_xml, "2").unwrap();
        let covers: Vec<_> =
            totk.images.iter().filter(|i| i.kind == "Box - Front").map(|i| (i.url.as_str(), i.region.as_str())).collect();
        assert_eq!(
            covers,
            [("https://images.launchbox-app.com/b.jpg", "North America"), ("https://images.launchbox-app.com/a.jpg", "Japan")]
        );
        assert!(totk.images.iter().any(|i| i.kind == "Clear Logo"));
    }

    #[test]
    fn reads_dates() {
        assert_eq!(date("1970-01-01"), 0);
        assert_eq!(date("2000-03-01T00:00:00Z"), 951_868_800);
        assert_eq!(date("not a date"), 0);
        assert_eq!(date(""), 0);
    }

    #[test]
    fn unknown_games_are_left_out() {
        assert!(lookup(scan_xml, &[want("x", &["Paper Dolls 2"], "")]).is_empty());
    }
}
