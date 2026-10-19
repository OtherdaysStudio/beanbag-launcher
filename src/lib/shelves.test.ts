import { describe, expect, it } from "vitest";
import { buildShelves } from "./shelves";
import type { Game } from "./types";

const game = (name: string, catalogs: string[] = [], lastPlayed = 0): Game => ({
  id: name,
  name,
  sortName: name,
  lastPlayed,
  playtime: 0,
  sizeBytes: 0,
  source: "",
  platform: "",
  description: "",
  genres: [],
  catalogs,
  hidden: false,
  added: false,
  installDir: "",
  popularity: 0,
  released: 0,
  art: {},
});

const ROWS = ["Story", "Couch co-op", "Online", "Indie"];
const games = [
  game("Hades", ["Indie"], 30),
  game("Celeste", ["Indie"]),
  game("Doom", ["Story"], 50),
  game("Portal 2", ["Couch co-op", "Couch picks"], 10),
  game("Apex", ["Online"]),
  game("Alyx", ["VR"]),
];

describe("buildShelves", () => {
  it("shows Recent, then the named rows in order, then everything", () => {
    const titles = buildShelves(games, "rows", 9, ROWS).map((s) => s.title);
    expect(titles).toEqual(["Recent", "Story", "Couch co-op", "Online", "Indie", "All games"]);
  });

  it("leaves out rows with no games and catalogs that are not rows", () => {
    const titles = buildShelves(games.filter((g) => g.name !== "Apex"), "rows", 9, ROWS).map((s) => s.title);
    expect(titles).not.toContain("Online");
    expect(titles).not.toContain("VR");
    expect(titles).not.toContain("Couch picks");
  });

  it("uses landscape tiles: 4 for Recent, 5 for the rest", () => {
    const shelves = buildShelves(games, "rows", 9, ROWS);
    expect(shelves.every((s) => s.wide)).toBe(true);
    expect(shelves.map((s) => s.visible)).toEqual([4, 5, 5, 5, 5, 5]);
  });

  it("orders a row by last played, then by name", () => {
    const indie = buildShelves(games, "rows", 9, ROWS).find((s) => s.title === "Indie")!;
    expect(indie.games.map((g) => g.name)).toEqual(["Hades", "Celeste"]);
  });

  it("sorts every row but Recent by the chosen order", () => {
    const pop = [game("Low", ["Indie"]), game("High", ["Indie"]), game("Mid", ["Indie"])];
    pop[0].popularity = 10;
    pop[1].popularity = 900;
    pop[2].popularity = 300;
    pop[0].released = 3;
    pop[1].released = 1;
    pop[2].released = 2;
    const row = (sort: "popular" | "released" | "az") => buildShelves(pop, "rows", 9, ROWS, sort).find((s) => s.title === "Indie")!.games.map((g) => g.name);
    expect(row("popular")).toEqual(["High", "Mid", "Low"]);
    expect(row("released")).toEqual(["Low", "Mid", "High"]);
    expect(row("az")).toEqual(["High", "Low", "Mid"]);
  });

  it("keeps the grid layout as poster rows of the screen width", () => {
    const grid = buildShelves(games, "grid", 4, ROWS, "az");
    expect(grid.map((s) => s.games.length)).toEqual([3, 4, 2]);
    expect(grid[1].title).toBe("Library");
  });

  it("shows nothing for an empty library", () => {
    expect(buildShelves([], "rows", 9, ROWS)).toEqual([]);
  });
});
