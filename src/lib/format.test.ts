import { describe, expect, it } from "vitest";
import { lastPlayed, playtime, size } from "./format";

const NOW = Date.UTC(2026, 9, 18, 12);
const daysAgo = (d: number) => Math.round((NOW - d * 86_400_000) / 1000);

describe("format", () => {
  it("says nothing for empty values", () => {
    expect(playtime(0)).toBe("");
    expect(size(0)).toBe("");
    expect(lastPlayed(0, NOW)).toBe("");
  });

  it("uses minutes under an hour and hours above", () => {
    expect(playtime(45)).toMatch(/45/);
    expect(playtime(45)).toMatch(/min/);
    expect(playtime(6120)).toMatch(/102/);
    expect(playtime(6120)).toMatch(/hour/);
  });

  it("describes when a game was last played", () => {
    expect(lastPlayed(daysAgo(0.1), NOW)).toMatch(/today/);
    expect(lastPlayed(daysAgo(1), NOW)).toMatch(/yesterday/);
    expect(lastPlayed(daysAgo(3), NOW)).toMatch(/3 days ago/);
    expect(lastPlayed(daysAgo(400), NOW)).toMatch(/year/);
  });
});
