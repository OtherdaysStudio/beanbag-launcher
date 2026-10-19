import type { Game } from "./types";

export function normalize(s: string): string {
  return s
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

/** Higher is better; 0 means no match. */
export function score(name: string, query: string): number {
  const n = normalize(name);
  const q = normalize(query);
  if (!q) return 0;
  const nc = n.replace(/ /g, "");
  const qc = q.replace(/ /g, "");
  const words = n.split(" ");

  if (n.startsWith(q) || nc.startsWith(qc)) return 100;
  if (words.some((w) => w.startsWith(q))) return 80;
  if (n.includes(q) || nc.includes(qc)) return 60;
  if (words.map((w) => w[0]).join("").startsWith(qc)) return 50;

  // Loose letter-order matches only once the query is long enough to mean something.
  if (qc.length < 3) return 0;
  let i = 0;
  for (const ch of nc) {
    if (ch === qc[i]) i++;
    if (i === qc.length) return 20;
  }
  return 0;
}

export function search(games: Game[], query: string): Game[] {
  return games
    .map((game) => ({ game, s: score(game.name, query) }))
    .filter((r) => r.s > 0)
    .sort((a, b) => b.s - a.s || b.game.lastPlayed - a.game.lastPlayed || byName(a.game, b.game))
    .map((r) => r.game);
}

/** A–Z by Playnite's sorting name, which already drops a leading "The". */
export function byName(a: Game, b: Game): number {
  return a.sortName.localeCompare(b.sortName, undefined, { numeric: true, sensitivity: "base" });
}
