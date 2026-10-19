//! Game names: comparing them and the other names a game may go by.

/// Sequel numerals as whole words: "Alan Wake II" matches "Alan Wake 2". I, V and X stay:
/// they are also letters and names ("I Am Bread", "Mega Man X").
const ROMAN: [(&str, &str); 7] = [("II", "2"), ("III", "3"), ("IV", "4"), ("VI", "6"), ("VII", "7"), ("VIII", "8"), ("IX", "9")];

/// Lower case letters and digits only, so "Director's Cut" matches "DIRECTOR'S CUT",
/// "Pokémon" matches "Pokemon" and "Alan Wake II" matches "Alan Wake 2".
pub fn normalize(name: &str) -> String {
    let folded: String = name.replace('&', " and ").chars().map(fold_accent).collect();
    folded
        .split(|c: char| !c.is_alphanumeric())
        .map(|word| ROMAN.iter().find(|(roman, _)| *roman == word).map_or(word, |(_, digit)| *digit))
        .collect::<String>()
        .to_lowercase()
}

fn fold_accent(c: char) -> char {
    match c {
        'à'..='å' | 'À'..='Å' => 'a',
        'è'..='ë' | 'È'..='Ë' => 'e',
        'ì'..='ï' | 'Ì'..='Ï' => 'i',
        'ò'..='ö' | 'Ò'..='Ö' => 'o',
        'ù'..='ü' | 'Ù'..='Ü' => 'u',
        'ç' | 'Ç' => 'c',
        'ñ' | 'Ñ' => 'n',
        _ => c,
    }
}

/// Words that name an edition of the same game, not a different game.
const EDITIONS: [&str; 15] = [
    "edition", "complete", "definitive", "remaster", "directorscut", "extendedcut", "goty", "gameoftheyear",
    "deluxe", "ultimate", "enhanced", "anniversary", "collection", "bundle", "parti",
];

pub fn is_edition(normalized_suffix: &str) -> bool {
    EDITIONS.iter().any(|e| normalized_suffix.contains(e))
}

/// Same game: equal names, or one is the other plus an edition ("Nioh 2" and
/// "Nioh 2 – The Complete Edition"). Never a sequel or a spin-off.
pub fn same_game(a: &str, b: &str) -> bool {
    let (a, b) = (normalize(a), normalize(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let (short, long) = if a.len() <= b.len() { (&a, &b) } else { (&b, &a) };
    match long.strip_prefix(short.as_str()) {
        Some("") => true,
        Some(rest) => is_edition(rest),
        None => false,
    }
}

/// Folder names that say nothing about the game inside.
const GENERIC_FOLDERS: [&str; 14] = [
    "emu", "emulator", "emulators", "games", "game", "bin", "binaries", "launcher", "cemu", "yuzu", "ryujinx",
    "rpcs3", "app", "win64",
];

/// "Night.Garden.Deluxe.v1.7.2-GOG" → "Night Garden Deluxe".
pub fn from_folder(install_dir: &str) -> Option<String> {
    let leaf = install_dir.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next()?;
    let parts: Vec<&str> = leaf.split(['.', '_', ' ', '-']).filter(|w| !w.is_empty()).collect();
    let digits = |w: &str| w.chars().all(|c| c.is_ascii_digit());
    let mut words = Vec::new();
    for (i, word) in parts.iter().enumerate() {
        let lower = word.to_lowercase();
        // "v1.9.6", or "10 3 3" (a version without the v); a single number stays ("Portal 2").
        let version = lower.strip_prefix('v').is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()))
            || (digits(word) && parts.get(i + 1).is_some_and(|next| digits(next)));
        let tag = ["gog", "multi", "x64"].contains(&lower.as_str());
        if version || tag {
            break;
        }
        words.push(*word);
    }
    let name = words.join(" ");
    let generic = GENERIC_FOLDERS.contains(&normalize(&name).as_str());
    (normalize(&name).len() >= 4 && !generic).then_some(name)
}

/// Words that mark a cut-down build, not the game. "free trial" comes before "trial", so
/// the longer one is cut whole.
const NOT_THE_GAME: [&str; 7] = ["demo", "playtest", "free trial", "trial", "beta", "alpha", "benchmark"];

/// The name to search art by: without a trailing "Demo", "- DEMO", "(Playtest)" and the like,
/// so "Blade Fury - DEMO" finds Blade Fury's art. A name that is only such a word stays.
pub fn search_name(name: &str) -> String {
    let separators = |c: char| c.is_whitespace() || "-–—:()[]_.".contains(c);
    let mut s = name.trim().to_string();
    let mut cut = false;
    loop {
        let t = s.trim_end_matches(separators);
        let rest = NOT_THE_GAME.iter().find_map(|w| {
            let at = t.len().checked_sub(w.len())?;
            let (rest, tail) = (t.get(..at)?, t.get(at..)?);
            (tail.eq_ignore_ascii_case(w) && rest.ends_with(|c: char| !c.is_alphanumeric()) && !rest.trim().is_empty())
                .then(|| rest.to_string())
        });
        match rest {
            Some(rest) => {
                s = rest;
                cut = true;
            }
            None if cut => return t.to_string(),
            None => return s,
        }
    }
}

/// The names to look a game up by, best first: its own name, the name without an
/// edition subtitle, the first game of a double pack, and its install folder. None of
/// them carries a "Demo" or "Playtest".
pub fn variants(name: &str, install_dir: &str) -> Vec<String> {
    let name = &search_name(name);
    let mut out = vec![name.to_string()];
    for sep in [": ", " - ", " – "] {
        if let Some((base, rest)) = name.split_once(sep) {
            if is_edition(&normalize(rest)) {
                out.push(base.to_string());
            }
        }
    }
    let pack = ["Double Pack", "Bundle"].iter().find_map(|s| name.strip_suffix(s)).map(str::trim);
    if let Some(first) = pack.and_then(|p| p.split(" & ").next()).and_then(|p| p.split(" and ").next()) {
        out.push(first.trim().to_string());
    }
    if let Some(folder) = from_folder(install_dir) {
        out.push(search_name(&folder));
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|v| !v.trim().is_empty() && seen.insert(normalize(v)));
    out
}

/// One short genre list for rows, whatever the source called it:
/// "Role-playing (RPG)" (Playnite) and "Role-Playing" (LaunchBox) are both "RPG".
pub fn genre(raw: &str) -> Option<&'static str> {
    let g = raw.to_lowercase();
    const MAP: [(&str, &str); 26] = [
        ("role", "RPG"),
        ("shoot", "Shooter"),
        ("fps", "Shooter"),
        ("platform", "Platformer"),
        ("puzzle", "Puzzle"),
        ("racing", "Racing"),
        ("driving", "Racing"),
        ("sport", "Sports"),
        ("tactic", "Strategy"),
        ("strateg", "Strategy"),
        ("fight", "Fighting"),
        ("beat", "Action"),
        ("hack and slash", "Action"),
        ("horror", "Horror"),
        ("survival", "Survival"),
        ("stealth", "Stealth"),
        ("point-and-click", "Adventure"),
        ("visual novel", "Visual Novel"),
        ("card", "Card & Board"),
        ("board", "Card & Board"),
        ("party", "Party"),
        ("music", "Music"),
        ("rhythm", "Music"),
        ("simulat", "Simulation"),
        ("sandbox", "Sandbox"),
        ("indie", "Indie"),
    ];
    if let Some((_, name)) = MAP.iter().find(|(key, _)| g.contains(key)) {
        return Some(name);
    }
    match g.trim() {
        "action" => Some("Action"),
        "adventure" => Some("Adventure"),
        "arcade" => Some("Arcade"),
        "casual" => Some("Casual"),
        "open world" => Some("Open World"),
        "roguelike" | "roguelite" => Some("Roguelike"),
        "metroidvania" => Some("Metroidvania"),
        _ => None,
    }
}

/// Mapped, without duplicates, in first-seen order.
pub fn genres<S: AsRef<str>>(raw: &[S]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for g in raw.iter().filter_map(|g| genre(g.as_ref())) {
        if !out.iter().any(|o| o == g) {
            out.push(g.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_case_accents_and_punctuation() {
        assert_eq!(normalize("Pokémon Sword & Shield"), "pokemonswordandshield");
        assert_eq!(normalize("Horizon Zero Dawn™"), "horizonzerodawn");
    }

    #[test]
    fn editions_are_the_same_game_but_sequels_are_not() {
        assert!(same_game("Nioh 2", "Nioh 2 – The Complete Edition"));
        assert!(same_game("The Falconeer", "The Falconeer: Revolution Remaster"));
        assert!(same_game("Horizon Zero Dawn", "Horizon Zero Dawn™ Complete Edition"));
        assert!(!same_game("Portal", "Portal 2"));
        assert!(!same_game("Tomb Raider", "Tomb Raider: Underworld"));
        assert!(!same_game("The Legend of Zelda", "The Legend of Zelda: Tears of the Kingdom"));
        assert!(!same_game("", "x"));
    }

    #[test]
    fn reads_game_names_from_folder_names() {
        assert_eq!(from_folder(r"D:\Games\Night.Garden.Deluxe.v1.7.2-GOG").as_deref(), Some("Night Garden Deluxe"));
        assert_eq!(from_folder(r"D:\Games\Quiet_Harbour_v1.9.6").as_deref(), Some("Quiet Harbour"));
        assert_eq!(from_folder(r"D:\Games\Paper Lantern 10 3 3").as_deref(), Some("Paper Lantern"));
        assert_eq!(from_folder(r"C:\Users\me\emu"), None);
        assert_eq!(from_folder(""), None);
    }

    #[test]
    fn maps_every_source_to_one_genre_list() {
        assert_eq!(genres(&["Role-playing (RPG)", "Role-Playing", "Adventure"]), ["RPG", "Adventure"]);
        assert_eq!(genres(&["Hack and slash/Beat 'em up", "Shooter"]), ["Action", "Shooter"]);
        assert_eq!(genres(&["Life Simulation", "Sandbox"]), ["Simulation", "Sandbox"]);
        assert!(genres(&["MMO", ""]).is_empty());
    }

    #[test]
    fn lists_lookup_names_best_first() {
        assert_eq!(
            variants("Mass Effect 2: Ultimate Edition", r"D:\Games\ME2"),
            ["Mass Effect 2: Ultimate Edition", "Mass Effect 2"]
        );
        assert_eq!(
            variants("Pokémon Sword & Pokémon Shield Double Pack", r"C:\emu"),
            ["Pokémon Sword & Pokémon Shield Double Pack", "Pokémon Sword"]
        );
        assert_eq!(variants("The Legend of Zelda: Tears of the Kingdom", ""), ["The Legend of Zelda: Tears of the Kingdom"]);
        assert_eq!(variants("Blade Fury - DEMO", r"D:\Games\Blade Fury Demo"), ["Blade Fury"]);
    }

    #[test]
    fn sequel_numerals_match_their_digits() {
        assert!(same_game("Alan Wake 2", "Alan Wake II"));
        assert_eq!(normalize("FINAL FANTASY VII"), normalize("Final Fantasy 7"));
        assert_eq!(normalize("Mega Man X"), "megamanx", "I, V and X are also letters and stay");
        assert_eq!(normalize("Vicious"), "vicious", "only whole words count");
        assert!(!same_game("Alan Wake II", "Alan Wake III"));
    }

    #[test]
    fn searches_without_demo_and_playtest_words() {
        assert_eq!(search_name("Blade Fury - DEMO"), "Blade Fury");
        assert_eq!(search_name("Hollow Knight: Silksong Demo"), "Hollow Knight: Silksong");
        assert_eq!(search_name("Game (Playtest)"), "Game");
        assert_eq!(search_name("Game [Free Trial]"), "Game");
        assert_eq!(search_name("Game Beta Demo"), "Game");
        assert_eq!(search_name("Alpha Protocol"), "Alpha Protocol");
        assert_eq!(search_name("Demolition Derby"), "Demolition Derby");
        assert_eq!(search_name("Demo"), "Demo");
        assert_eq!(search_name("Game (2019)"), "Game (2019)");
    }
}
