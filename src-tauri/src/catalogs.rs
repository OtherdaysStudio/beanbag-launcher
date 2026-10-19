//! The rows on the big screen, filled from what Steam knows about each game
//! (player modes and tags) and, for games Steam does not sell, from LaunchBox.

pub const STORY: &str = "Story";
pub const INDIE: &str = "Indie";
pub const COUCH: &str = "Couch co-op";
pub const ONLINE: &str = "Online";
pub const VR: &str = "VR";

/// Big-screen order. VR games keep their catalog for desktop mode but get no row:
/// most people play from the couch without a headset.
pub const ORDER: [&str; 4] = [STORY, COUCH, ONLINE, INDIE];

/// Rows renamed since 0.1; catalogs saved under the old names follow them.
pub fn renamed(name: &str) -> &str {
    match name {
        "AAA story" => STORY,
        "Same screen" => COUCH,
        "Multiplayer" => ONLINE,
        other => other,
    }
}

/// Steam feature category for headset games ("VR Supported").
const VR_FEATURE: u32 = 54;

/// Steam player categories for one screen and one PC: Shared/Split Screen, its PvP and Co-op.
const LOCAL_PLAYERS: [u32; 3] = [24, 37, 39];
/// Steam player categories for online play against others: MMO, Online PvP, PvP.
const ONLINE_PVP: [u32; 3] = [20, 36, 49];
/// Steam player category for online co-op.
const ONLINE_COOP: u32 = 38;

const LOCAL_TAGS: [&str; 5] = ["Local Co-Op", "Local Multiplayer", "Split Screen", "Couch Co-Op", "4 Player Local"];
/// Tags that mean playing with people online. Plain "Multiplayer" and "Co-op" are left out:
/// party games on one screen carry them too.
const ONLINE_TAGS: [&str; 10] = [
    "Battle Royale",
    "PvP",
    "Massively Multiplayer",
    "MMO",
    "Online Co-Op",
    "Team-Based",
    "Competitive",
    "eSports",
    "Hero Shooter",
    "Extraction Shooter",
];
/// Apps on Steam that are not games.
const TOOL_TAGS: [&str; 5] = ["Utilities", "Software", "Benchmark", "Design & Illustration", "Video Production"];
const STORY_TAGS: [&str; 5] = ["Story Rich", "Singleplayer", "Narrative", "Choices Matter", "Cinematic"];

/// What is known about how a game is played.
#[derive(Default)]
pub struct Traits<'a> {
    /// Steam tags, strongest first.
    pub tags: &'a [String],
    /// Steam supported-player category ids.
    pub players: &'a [u32],
    /// Steam feature category ids.
    pub features: &'a [u32],
    /// LaunchBox's player count, for games Steam does not sell.
    pub max_players: u32,
    pub platform: &'a str,
    pub genres: &'a [String],
}

fn rank(tags: &[String], names: &[&str]) -> Option<usize> {
    tags.iter().position(|t| names.iter().any(|n| t.eq_ignore_ascii_case(n)))
}

pub fn classify(t: &Traits) -> Vec<&'static str> {
    // Tools (SteamVR, benchmarks) stay in All games and out of every row.
    if rank(t.tags, &TOOL_TAGS).is_some_and(|r| r < 3) {
        return Vec::new();
    }
    // A headset game belongs only in VR: it is no use from the couch.
    // A weak "VR" tag is not enough (Resident Evil 4 has one for its PlayStation VR mode).
    if t.features.contains(&VR_FEATURE) || rank(t.tags, &["VR", "VR Only"]).is_some_and(|r| r < 3) {
        return vec![VR];
    }
    let from_steam = !t.tags.is_empty() || !t.players.is_empty();
    let console = !t.platform.is_empty() && !t.platform.to_lowercase().contains("windows") && !t.platform.starts_with("PC");

    let indie = rank(t.tags, &["Indie"]).is_some_and(|r| r < 15);
    let local = t.players.iter().any(|c| LOCAL_PLAYERS.contains(c))
        || rank(t.tags, &LOCAL_TAGS).is_some_and(|r| r < 15)
        || (!from_steam && console && t.max_players >= 2);

    // "Multiplayer" is playing with people online, and what the game is known for: an online
    // tag in its top five, above any story tag (Baldur's Gate 3 has online co-op, but people
    // tag it for its story first). A couch game whose only online side is co-op stays in
    // Couch co-op.
    let online = rank(t.tags, &ONLINE_TAGS);
    let story = rank(t.tags, &STORY_TAGS);
    let pvp = t.players.iter().any(|c| ONLINE_PVP.contains(c));
    let known_for_online = match (online, story) {
        (Some(m), Some(s)) => m < 5 && m < s,
        (Some(m), None) => m < 5,
        (None, _) => pvp && story.is_none() && rank(t.tags, &["Multiplayer"]).is_some_and(|r| r < 5),
    };
    let couch_only = local && !pvp && rank(t.tags, &["Battle Royale", "MMO", "Massively Multiplayer"]).is_none();
    let multiplayer = known_for_online && !couch_only && (pvp || t.players.contains(&ONLINE_COOP) || online.is_some());

    let big_story = if from_steam {
        story.is_some_and(|s| s < 12)
    } else {
        // Console games from LaunchBox: single-player adventures and RPGs.
        console && t.max_players <= 1 && t.genres.iter().any(|g| matches!(g.as_str(), "Adventure" | "RPG"))
    };

    let mut out = Vec::new();
    if big_story && !indie && !multiplayer {
        out.push(STORY);
    }
    if indie {
        out.push(INDIE);
    }
    if local {
        out.push(COUCH);
    }
    if multiplayer {
        out.push(ONLINE);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn steam(t: &[&str], players: &[u32]) -> Vec<&'static str> {
        let t = tags(t);
        classify(&Traits { tags: &t, players, ..Default::default() })
    }

    #[test]
    fn headset_games_go_only_to_vr() {
        let alyx = tags(&["VR", "Story Rich", "Singleplayer"]);
        assert_eq!(classify(&Traits { tags: &alyx, features: &[22, 52, 54, 31], ..Default::default() }), [VR]);
        let skyrim = tags(&["Open World", "RPG", "VR"]);
        assert_eq!(classify(&Traits { tags: &skyrim, ..Default::default() }), [VR]);
        let re4 = tags(&["Horror", "Action", "Survival Horror", "Third-Person Shooter", "Singleplayer", "Zombies", "Story Rich", "Survival", "Remake", "Dark", "Cinematic", "Gore", "Shooter", "VR"]);
        assert_eq!(classify(&Traits { tags: &re4, features: &[22, 29, 23, 61, 62], players: &[2], ..Default::default() }), [STORY]);
    }

    #[test]
    fn real_steam_games_land_in_the_right_rows() {
        let helldivers = steam(&["Online Co-Op", "PvE", "Third-Person Shooter", "Multiplayer", "Shooter"], &[1, 9, 38]);
        assert_eq!(helldivers, [ONLINE]);
        let bg3 = steam(
            &["RPG", "Character Customization", "Choices Matter", "Turn-Based Combat", "Story Rich", "Sexual Content", "CRPG", "Fantasy", "Adventure", "Romance", "Online Co-Op", "Multiplayer"],
            &[2, 1, 9, 38, 48, 27],
        );
        assert_eq!(bg3, [STORY]);
        let stardew = steam(&["Farming Sim", "Pixel Graphics", "Multiplayer", "Life Sim", "RPG", "Relaxing", "Simulation", "Agriculture", "Crafting", "Sandbox", "Indie"], &[2, 1, 9, 38, 48, 39, 24]);
        assert_eq!(stardew, [INDIE, COUCH]);
        let apex = steam(&["Free to Play", "Multiplayer", "Battle Royale", "FPS", "Shooter"], &[1, 49, 36, 9, 38]);
        assert_eq!(apex, [ONLINE]);
        let re4 = steam(&["Horror", "Action", "Survival Horror", "Third-Person Shooter", "Singleplayer", "Zombies", "Story Rich"], &[2]);
        assert_eq!(re4, [STORY]);
        let celeste = steam(&["Precision Platformer", "Difficult", "Pixel Graphics", "Great Soundtrack", "Indie", "Platformer", "2D Platformer", "Female Protagonist", "Singleplayer", "2D", "Story Rich"], &[2]);
        assert_eq!(celeste, [INDIE]);
    }

    #[test]
    fn party_and_couch_games_are_same_screen_not_multiplayer() {
        let jackbox = steam(&["Party Game", "Multiplayer", "Funny", "Local Multiplayer", "Comedy"], &[1, 24, 37]);
        assert_eq!(jackbox, [COUCH]);
        let broforce = steam(&["Co-op", "Action", "Local Co-Op", "Multiplayer", "Online Co-Op", "Indie"], &[2, 1, 9, 38, 39, 24]);
        assert_eq!(broforce, [INDIE, COUCH]);
    }

    #[test]
    fn tools_stay_out_of_every_row() {
        assert!(steam(&["Utilities", "VR", "Free to Play"], &[2]).is_empty());
        assert!(steam(&["Benchmark", "Software"], &[]).is_empty());
    }

    #[test]
    fn online_co_op_alone_is_not_same_screen() {
        assert!(!steam(&["Online Co-Op"], &[1, 9, 38]).contains(&COUCH));
        assert!(steam(&["Racing", "Split Screen"], &[1, 2]).contains(&COUCH));
    }

    #[test]
    fn console_games_use_the_launchbox_player_count() {
        let racing = tags(&["Racing"]);
        let kart = classify(&Traits { max_players: 4, platform: "Nintendo Switch", genres: &racing, ..Default::default() });
        assert_eq!(kart, [COUCH]);
        let adventure = tags(&["Adventure", "RPG"]);
        let zelda = classify(&Traits { max_players: 1, platform: "Nintendo Switch", genres: &adventure, ..Default::default() });
        assert_eq!(zelda, [STORY]);
        let pc = classify(&Traits { max_players: 4, platform: "Windows", genres: &racing, ..Default::default() });
        assert!(pc.is_empty());
    }
}
