export interface GameArt {
  cover?: string;
  /** Wide background art. */
  hero?: string;
  /** Transparent title logo. */
  logo?: string;
}

export interface Game {
  /** Playnite's id for imported games, "steam-<appid>" for Steam games found later. */
  id: string;
  name: string;
  sortName: string;
  /** Unix seconds; 0 when never played. */
  lastPlayed: number;
  /** Minutes. */
  playtime: number;
  sizeBytes: number;
  /** "Steam", "Epic", "" for games added by hand. */
  source: string;
  /** "PC (Windows)", "Nintendo Switch", … */
  platform: string;
  description: string;
  /** One short list across sources: "RPG", "Shooter", … */
  genres: string[];
  /** Playnite categories and catalogs made in Beanbag. */
  catalogs: string[];
  hidden: boolean;
  /** Added in Beanbag rather than imported. */
  added: boolean;
  installDir: string;
  /** Steam review count, or LaunchBox rating count. */
  popularity: number;
  /** Unix seconds; 0 when unknown. */
  released: number;
  art: GameArt;
}

export interface Library {
  /** False when there is no import and no Steam. */
  found: boolean;
  games: Game[];
  /** The big-screen rows, in order: "Story", "Couch co-op", "Online", "Indie" */
  rows: string[];
}

export type Action =
  | "up"
  | "down"
  | "left"
  | "right"
  | "a"
  | "b"
  | "x"
  | "y"
  | "menu"
  | "lb"
  | "rb";

export type Direction = "up" | "down" | "left" | "right";
