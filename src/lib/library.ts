import type { Game, Library } from "./types";

const CACHE_KEY = "beanbag.library.v4";

export const isTauri = "__TAURI_INTERNALS__" in window;

/** The last library this machine saw, so the first frame needs no disk read. */
export function cachedLibrary(): Library | null {
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    return raw ? (JSON.parse(raw) as Library) : null;
  } catch {
    return null;
  }
}

export async function loadLibrary(): Promise<Library> {
  const library = isTauri ? await readLibrary() : sampleLibrary();
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(library));
  } catch {
    // Storage full or blocked: the next start reads the file again.
  }
  return library;
}

/** One game as the Rust side returns it: the Playnite import plus Steam, with Beanbag's stats. */
interface StoredGame {
  id: string;
  name: string;
  sortName: string;
  lastPlayed: number;
  /** Seconds. */
  playtime: number;
  sizeBytes: number;
  source: string;
  platform: string;
  description: string;
  genres: string[];
  catalogs: string[];
  hidden: boolean;
  added: boolean;
  installDir: string;
  popularity: number;
  released: number;
  cover: string | null;
  background: string | null;
  logo: string | null;
}

async function readLibrary(): Promise<Library> {
  const { invoke, convertFileSrc } = await import("@tauri-apps/api/core");
  const raw = await invoke<{ found: boolean; games: StoredGame[]; rows: string[] }>("load_library");
  const src = (path: string | null) =>
    !path ? undefined : /^https?:\/\//.test(path) ? path : convertFileSrc(path);
  return {
    found: raw.found,
    rows: raw.rows,
    games: raw.games.map((g) => ({
      id: g.id,
      name: g.name,
      sortName: g.sortName || g.name,
      lastPlayed: g.lastPlayed,
      playtime: Math.round(g.playtime / 60),
      sizeBytes: g.sizeBytes,
      source: g.source,
      platform: g.platform,
      description: g.description,
      genres: g.genres,
      catalogs: g.catalogs,
      hidden: g.hidden,
      added: g.added,
      installDir: g.installDir,
      popularity: g.popularity,
      released: g.released,
      art: { cover: src(g.cover), hero: src(g.background), logo: src(g.logo) },
    })),
  };
}

/** Browser preview only: real games, art from the Steam CDN, invented stats. */
function sampleLibrary(): Library {
  const cdn = "https://cdn.cloudflare.steamstatic.com/steam/apps";
  const now = Math.floor(Date.now() / 1000);
  const day = 86_400;
  const blurb =
    "Gather your party and return to the Forgotten Realms in a tale of fellowship and betrayal, sacrifice and survival, and the lure of absolute power.";
  const rows: Array<[number, string, number, number, string[], string[]]> = [
    [1086940, "Baldur's Gate 3", 0.2, 6120, ["RPG", "Strategy"], ["Story"]],
    [2379780, "Balatro", 1, 2900, ["Card & Board", "Roguelike"], ["Indie"]],
    [1145360, "Hades", 3, 2210, ["Action", "Roguelike"], ["Indie"]],
    [1245620, "ELDEN RING", 6, 9400, ["RPG", "Action"], ["Story"]],
    [553850, "HELLDIVERS 2", 9, 1830, ["Shooter", "Action"], ["Online"]],
    [413150, "Stardew Valley", 14, 7300, ["Simulation", "RPG"], ["Indie", "Couch co-op", "Online"]],
    [367520, "Hollow Knight", 21, 1520, ["Platformer", "Action"], ["Indie"]],
    [1091500, "Cyberpunk 2077", 35, 4100, ["RPG", "Shooter"], ["Story"]],
    [292030, "The Witcher 3: Wild Hunt", 60, 8800, ["RPG", "Action"], ["Story"]],
    [1174180, "Red Dead Redemption 2", 90, 5200, ["Action", "Adventure"], ["Story"]],
    [620, "Portal 2", 200, 840, ["Puzzle", "Platformer"], ["Couch co-op"]],
    [1593500, "God of War", 120, 2600, ["Action", "Adventure"], ["Story"]],
    [2050650, "Resident Evil 4", 45, 1300, ["Horror", "Shooter"], ["Story"]],
    [814380, "Sekiro: Shadows Die Twice", 300, 3100, ["Action", "Adventure"], ["Story"]],
    [105600, "Terraria", 400, 6400, ["Sandbox", "Action"], ["Indie", "Online"]],
    [504230, "Celeste", 500, 900, ["Platformer"], ["Indie"]],
    [646570, "Slay the Spire", 30, 4300, ["Card & Board", "Roguelike", "Strategy"], ["Indie"]],
    [1794680, "Vampire Survivors", 12, 1100, ["Action", "Roguelike"], ["Indie", "Couch co-op"]],
    [548430, "Deep Rock Galactic", 70, 2400, ["Shooter"], ["Online"]],
    [1888930, "The Last of Us Part I", 160, 1700, ["Action", "Horror"], ["Story"]],
    [1817070, "Marvel's Spider-Man Remastered", 110, 1900, ["Action", "Adventure"], ["Story"]],
    [990080, "Hogwarts Legacy", 0, 0, ["RPG", "Adventure"], ["Story"]],
    [271590, "Grand Theft Auto V", 700, 12000, ["Action", "Shooter"], ["Online"]],
    [546560, "Half-Life: Alyx", 400, 1200, ["Shooter"], ["VR"]],
  ];
  const games: Game[] = rows.map(([appId, name, daysAgo, playtime, genres, catalogs]) => ({
    id: `sample-${appId}`,
    name,
    sortName: name.replace(/^The /, ""),
    lastPlayed: playtime ? now - Math.round(daysAgo * day) : 0,
    playtime,
    sizeBytes: (20 + (appId % 130)) * 1e9,
    source: "Steam",
    platform: "PC (Windows)",
    description: appId === 1086940 ? blurb : "",
    genres,
    catalogs,
    hidden: false,
    added: false,
    installDir: "",
    popularity: (appId % 997) * 1000,
    released: now - (appId % 4000) * day,
    art: {
      cover: `${cdn}/${appId}/library_600x900_2x.jpg`,
      hero: `${cdn}/${appId}/library_hero.jpg`,
      logo: `${cdn}/${appId}/logo.png`,
    },
  }));
  games.push({
    id: "sample-none",
    name: "Kirby's Return to Dream Land Deluxe",
    sortName: "Kirby's Return to Dream Land Deluxe",
    lastPlayed: 0,
    playtime: 0,
    sizeBytes: 0,
    source: "",
    platform: "Nintendo Switch",
    description: "",
    genres: ["Platformer"],
    catalogs: ["Couch co-op"],
    hidden: false,
    added: false,
    installDir: "",
    popularity: 0,
    released: 0,
    art: {},
  });
  return { found: true, games, rows: ["Story", "Couch co-op", "Online", "Indie"] };
}
