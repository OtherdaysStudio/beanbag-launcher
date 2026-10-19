import { describe, expect, it } from "vitest";
import { chunk, clamp, columnsFor, move, moveShelf, rowOffset } from "./nav";

// Recent row of 4 tiles, then a 9-wide grid whose last row has 3.
const rows = [4, 9, 9, 3];

describe("move", () => {
  it("stops at every edge instead of wrapping", () => {
    expect(move({ row: 0, col: 0 }, "left", rows)).toMatchObject({ row: 0, col: 0 });
    expect(move({ row: 0, col: 0 }, "up", rows)).toMatchObject({ row: 0, col: 0 });
    expect(move({ row: 1, col: 8 }, "right", rows)).toMatchObject({ row: 1, col: 8 });
    expect(move({ row: 3, col: 2 }, "down", rows)).toMatchObject({ row: 3, col: 2 });
  });

  it("lands on the nearest card in a shorter row", () => {
    expect(move({ row: 1, col: 7 }, "up", rows)).toMatchObject({ row: 0, col: 3 });
    expect(move({ row: 2, col: 8 }, "down", rows)).toMatchObject({ row: 3, col: 2 });
  });

  it("remembers the column across a shorter row", () => {
    const down = move({ row: 2, col: 8 }, "down", rows);
    expect(move(down, "up", rows)).toMatchObject({ row: 2, col: 8 });
    const up = move({ row: 1, col: 6 }, "up", rows);
    expect(move(up, "down", rows)).toMatchObject({ row: 1, col: 6 });
  });

  it("forgets the remembered column after a sideways move", () => {
    const down = move({ row: 2, col: 8 }, "down", rows);
    const left = move(down, "left", rows);
    expect(move(left, "up", rows)).toMatchObject({ row: 2, col: 1 });
  });
});

describe("clamp", () => {
  it("pulls focus back inside rows that shrank", () => {
    expect(clamp({ row: 9, col: 9 }, rows)).toMatchObject({ row: 3, col: 2 });
    expect(clamp({ row: 2, col: 4 }, [])).toEqual({ row: 0, col: 0 });
  });
});

describe("layout", () => {
  it("fits 9 posters on a 1920 screen and never fewer than 4", () => {
    expect(columnsFor(1920)).toBe(9);
    expect(columnsFor(2560)).toBe(10);
    expect(columnsFor(600)).toBe(4);
  });

  it("chunks a list into rows", () => {
    expect(chunk([1, 2, 3, 4, 5], 2)).toEqual([[1, 2], [3, 4], [5]]);
    expect(chunk([], 3)).toEqual([]);
  });
});

describe("rowOffset", () => {
  it("scrolls only when focus reaches the last visible card", () => {
    expect(rowOffset(0, 7, 30, 9)).toBe(0);
    expect(rowOffset(0, 8, 30, 9)).toBe(1);
    expect(rowOffset(5, 4, 30, 9)).toBe(4);
  });

  it("never scrolls a row that fits, or past its end", () => {
    expect(rowOffset(0, 5, 6, 9)).toBe(0);
    expect(rowOffset(0, 29, 30, 9)).toBe(21);
  });
});

describe("moveShelf", () => {
  const rows = [4, 30, 12];
  const visible = (r: number) => (r === 0 ? 4 : 9);

  it("scrolls a row sideways as focus moves along it", () => {
    let s = { focus: { row: 1, col: 0 }, offsets: [0, 0, 0] };
    for (let i = 0; i < 10; i++) s = moveShelf(s, "right", rows, visible);
    expect(s.focus).toEqual({ row: 1, col: 10 });
    expect(s.offsets[1]).toBe(3);
  });

  it("lands on the card at the same place on screen in the next row", () => {
    const s = moveShelf({ focus: { row: 1, col: 10 }, offsets: [0, 3, 0] }, "down", rows, visible);
    expect(s.focus).toEqual({ row: 2, col: 7 });
    expect(s.offsets[2]).toBe(0);
  });

  it("keeps each row where it was left", () => {
    const s = moveShelf({ focus: { row: 2, col: 2 }, offsets: [0, 3, 0] }, "up", rows, visible);
    expect(s.focus).toEqual({ row: 1, col: 5 });
    expect(s.offsets[1]).toBe(3);
  });

  it("clamps to a shorter row", () => {
    const s = moveShelf({ focus: { row: 1, col: 8 }, offsets: [0, 0, 0] }, "up", rows, visible);
    expect(s.focus).toEqual({ row: 0, col: 3 });
  });
});
