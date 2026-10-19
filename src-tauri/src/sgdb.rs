//! SteamGridDB: community covers, backgrounds and logos, many for each game. It needs a
//! personal API key, which lives in `settings.json` on this PC and never in the source.

use crate::art::Kind;
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

const API: &str = "https://www.steamgriddb.com/api/v2";
pub const SETTINGS_FILE: &str = "settings.json";
/// Enough to choose from without a wall of thumbnails.
const PER_KIND: usize = 30;

#[derive(Deserialize, Serialize, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub steam_grid_db_key: Option<String>,
}

pub fn read_settings(data: &Path) -> Settings {
    fs::read_to_string(data.join(SETTINGS_FILE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn write_settings(data: &Path, settings: &Settings) -> io::Result<()> {
    let tmp = data.join(format!("{SETTINGS_FILE}.tmp"));
    fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
    fs::rename(tmp, data.join(SETTINGS_FILE))
}

/// The saved key, if there is one.
pub fn key(data: &Path) -> Option<String> {
    read_settings(data).steam_grid_db_key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
}

/// A game on SteamGridDB, by its own id or by a Steam app id.
#[derive(Clone, Copy, Debug)]
pub enum Game {
    Id(u64),
    Steam(u32),
}

/// One image: full size, and a small copy for the picker.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub url: String,
    pub thumb: String,
}

pub struct Client<'a> {
    agent: &'a ureq::Agent,
    key: &'a str,
}

impl<'a> Client<'a> {
    pub fn new(agent: &'a ureq::Agent, key: &'a str) -> Self {
        Client { agent, key }
    }

    fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<serde_json::Value, String> {
        let mut req = self.agent.get(format!("{API}{path}")).header("Authorization", format!("Bearer {}", self.key));
        for (k, v) in query {
            req = req.query(*k, *v);
        }
        match req.call() {
            Ok(mut resp) => resp.body_mut().read_json().map_err(|e| e.to_string()),
            Err(ureq::Error::StatusCode(401)) => Err("SteamGridDB did not accept the key.".into()),
            Err(ureq::Error::StatusCode(404)) => Ok(serde_json::Value::Null),
            Err(e) => Err(format!("SteamGridDB did not answer ({e}).")),
        }
    }

    /// Games whose names match, best first.
    pub fn search(&self, query: &str) -> Result<Vec<(u64, String)>, String> {
        let v = self.get(&format!("/search/autocomplete/{}", encode(query)), &[])?;
        Ok(v["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|g| Some((g["id"].as_u64()?, g["name"].as_str()?.to_string())))
            .collect())
    }

    /// Still images only, nothing marked adult or joke.
    pub fn images(&self, game: Game, kind: Kind) -> Result<Vec<Image>, String> {
        let what = match kind {
            Kind::Cover => "grids",
            Kind::Background => "heroes",
            Kind::Logo => "logos",
        };
        let path = match game {
            Game::Id(id) => format!("/{what}/game/{id}"),
            Game::Steam(app) => format!("/{what}/steam/{app}"),
        };
        let mut query = vec![("types", "static"), ("nsfw", "false"), ("humor", "false")];
        if kind == Kind::Cover {
            // Portrait covers only; SteamGridDB also has wide and square ones.
            query.push(("dimensions", "600x900,342x482,660x930"));
        }
        Ok(parse_images(&self.get(&path, &query)?))
    }
}

fn parse_images(v: &serde_json::Value) -> Vec<Image> {
    v["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| {
            let url = i["url"].as_str()?.to_string();
            let thumb = i["thumb"].as_str().map(String::from).unwrap_or_else(|| url.clone());
            Some(Image { url, thumb })
        })
        .take(PER_KIND)
        .collect()
}

/// A name as one part of a URL path.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_images_and_falls_back_to_the_full_size_for_a_thumbnail() {
        let v = serde_json::json!({ "success": true, "data": [
            { "id": 1, "url": "https://cdn/a.png", "thumb": "https://cdn/thumb/a.png" },
            { "id": 2, "url": "https://cdn/b.png" },
            { "id": 3 }
        ]});
        assert_eq!(
            parse_images(&v),
            [
                Image { url: "https://cdn/a.png".into(), thumb: "https://cdn/thumb/a.png".into() },
                Image { url: "https://cdn/b.png".into(), thumb: "https://cdn/b.png".into() },
            ]
        );
        assert!(parse_images(&serde_json::Value::Null).is_empty());
    }

    #[test]
    fn encodes_a_name_for_the_path() {
        assert_eq!(encode("Assassin's Creed II"), "Assassin%27s%20Creed%20II");
        assert_eq!(encode("Pokémon/X"), "Pok%C3%A9mon%2FX");
    }

    #[test]
    fn keeps_the_key_in_settings_and_trims_it() {
        let data = std::env::temp_dir().join(format!("beanbag-sgdb-{}", std::process::id()));
        fs::create_dir_all(&data).unwrap();
        assert_eq!(key(&data), None);
        write_settings(&data, &Settings { steam_grid_db_key: Some("  abc123 \n".into()) }).unwrap();
        assert_eq!(key(&data).as_deref(), Some("abc123"));
        write_settings(&data, &Settings { steam_grid_db_key: Some("   ".into()) }).unwrap();
        assert_eq!(key(&data), None);
        fs::remove_dir_all(data).unwrap();
    }
}
