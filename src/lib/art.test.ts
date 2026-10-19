import { describe, expect, it } from "vitest";
import { matchLine } from "./art";
import type { Choice, Choices, Match } from "./platform";

const steam: Match = { source: "steam", id: "990080", name: "Hogwarts Legacy", platform: "Steam", image: null };
const pick = (source: string): Choice => ({ url: `https://x/${source}`, thumb: "", source });
const choices = (...sources: string[]): Choices => ({
  cover: sources.map(pick),
  background: [],
  logo: [],
  description: "",
  genres: [],
  note: null,
});

describe("match line", () => {
  it("names every source the images came from", () => {
    expect(matchLine("Hogwarts Legacy", steam, choices("Steam", "SteamGridDB", "SteamGridDB"))).toBe("Images from Steam and SteamGridDB");
    expect(matchLine("Hogwarts Legacy", steam, choices("Steam"))).toBe("Images from Steam");
  });

  it("reads LaunchBox's regions as one source", () => {
    const lb: Match = { ...steam, source: "launchbox" };
    expect(matchLine("Hogwarts Legacy", lb, choices("LaunchBox · Europe", "LaunchBox", "SteamGridDB"))).toBe(
      "Images from LaunchBox and SteamGridDB",
    );
  });

  it("names the matched game only when it is another name", () => {
    expect(matchLine("Blade Fury - DEMO", { ...steam, name: "Blade Fury" }, choices("Steam"))).toBe("Images for Blade Fury from Steam");
  });

  it("falls back to the match's own source before the images arrive", () => {
    expect(matchLine("Hogwarts Legacy", steam, null)).toBe("Images from Steam");
  });
});
