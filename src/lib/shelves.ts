import type { Shelf } from "../components/Shelves";
import { chunk } from "./nav";
import { byName } from "./search";
import type { Game } from "./types";

export type Layout = "rows" | "grid";
export type Sort = "recent" | "az" | "popular" | "released";

export const SORTS: ReadonlyArray<[Sort, string]> = [
  ["recent", "Recently played"],
  ["az", "A–Z"],
  ["popular", "Popularity"],
  ["released", "Release date"],
];

const RECENT = 4;

const byRecent = (a: Game, b: Game) => b.lastPlayed - a.lastPlayed;

/** Each sort falls back to A–Z, so games with no data stay in a calm order. */
const ORDER: Record<Sort, (a: Game, b: Game) => number> = {
  recent: (a, b) => byRecent(a, b) || byName(a, b),
  az: byName,
  popular: (a, b) => b.popularity - a.popularity || byName(a, b),
  released: (a, b) => b.released - a.released || byName(a, b),
};

/**
 * Rows: Recent, then each named row in order (empty ones left out), then everything.
 * All landscape: Recent shows 4 large tiles, the rest 5.
 * Grid: Recent, then the whole library in posters, `cols` to a row.
 * `sort` orders every row except Recent.
 */
export function buildShelves(games: Game[], layout: Layout, cols: number, rows: string[], sort: Sort = "recent"): Shelf[] {
  const recent = games.filter((g) => g.lastPlayed > 0).sort(byRecent).slice(0, RECENT);
  const order = ORDER[sort];
  const az = [...games].sort(order);
  const top: Shelf[] = recent.length ? [{ key: "recent", title: "Recent", games: recent, wide: true, visible: 4 }] : [];

  if (layout === "grid") {
    return [...top, ...chunk(az, cols).map((g, i) => ({ key: `grid-${i}`, title: i === 0 ? "Library" : "", games: g }))];
  }

  const named = rows
    .map((title) => ({ key: `row:${title}`, title, games: games.filter((g) => g.catalogs.includes(title)).sort(order), wide: true, visible: 5 }))
    .filter((s) => s.games.length > 0);
  const all = az.length ? [{ key: "all", title: "All games", games: az, wide: true, visible: 5 }] : [];
  return [...top, ...named, ...all];
}
