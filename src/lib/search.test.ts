import { describe, expect, it } from "vitest";
import { byName, score, search } from "./search";
import type { Game } from "./types";

const game = (name: string, lastPlayed = 0, sortName = name): Game => ({
  id: name,
  name,
  sortName,
  lastPlayed,
  playtime: 0,
  sizeBytes: 0,
  source: "",
  platform: "",
  description: "",
  genres: [],
  catalogs: [],
  hidden: false,
  added: false,
  installDir: "",
  popularity: 0,
  released: 0,
  art: {},
});

describe("score", () => {
  it("ranks a prefix above a word start above a substring", () => {
    expect(score("Hollow Knight", "hol")).toBeGreaterThan(score("The Witcher", "wit"));
    expect(score("The Witcher", "wit")).toBeGreaterThan(score("Hades", "ade"));
  });

  it("ignores case, accents and punctuation", () => {
    expect(score("Pokémon Legends: Arceus", "pokemon legends arceus")).toBe(100);
    expect(score("Marvel's Spider-Man", "marvels spiderman")).toBe(100);
  });

  it("matches initials", () => {
    expect(score("Red Dead Redemption 2", "rdr")).toBeGreaterThan(0);
  });

  it("does not match loose letters from short queries", () => {
    expect(score("Grand Theft Auto V", "ho")).toBe(0);
    expect(score("Grand Theft Auto V", "gtav")).toBeGreaterThan(0);
  });

  it("returns 0 for an empty query", () => {
    expect(score("Anything", "  ")).toBe(0);
  });
});

describe("search", () => {
  it("puts the best match first, then the most recently played", () => {
    const games = [game("Portal 2", 10), game("Hades", 5), game("Hades II", 50)];
    expect(search(games, "had").map((g) => g.name)).toEqual(["Hades II", "Hades"]);
  });
});

describe("byName", () => {
  it("sorts by sorting name with numbers in order", () => {
    const games = [game("Portal 10"), game("The Witcher 3", 0, "Witcher 3"), game("Portal 2")];
    expect(games.sort(byName).map((g) => g.name)).toEqual(["Portal 2", "Portal 10", "The Witcher 3"]);
  });
});
