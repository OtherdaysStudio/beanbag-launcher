import type { Direction } from "./types";

/** A position in a stack of rows. `want` keeps the column across shorter rows. */
export interface Focus {
  row: number;
  col: number;
  want?: number;
}

/** Moves focus one step. `rows` holds the length of each row; empty rows are not allowed. */
export function move(focus: Focus, dir: Direction, rows: number[]): Focus {
  if (rows.length === 0) return focus;
  const { row, col } = clamp(focus, rows);
  const want = focus.want ?? col;

  switch (dir) {
    case "left":
      return { row, col: Math.max(0, col - 1) };
    case "right":
      return { row, col: Math.min(rows[row] - 1, col + 1) };
    case "up":
      if (row === 0) return { row, col, want };
      return { row: row - 1, col: Math.min(want, rows[row - 1] - 1), want };
    case "down":
      if (row === rows.length - 1) return { row, col, want };
      return { row: row + 1, col: Math.min(want, rows[row + 1] - 1), want };
  }
}

/** Keeps focus inside the rows after the rows change (rescan, resize). */
export function clamp(focus: Focus, rows: number[]): Focus {
  if (rows.length === 0) return { row: 0, col: 0 };
  const row = Math.min(Math.max(0, focus.row), rows.length - 1);
  const col = Math.min(Math.max(0, focus.col), rows[row] - 1);
  return { row, col, want: focus.want };
}

export function same(a: Focus, b: Focus): boolean {
  return a.row === b.row && a.col === b.col;
}

/** Poster columns for a viewport width: 9 at 1920, never fewer than 4. */
export function columnsFor(width: number): number {
  return Math.min(10, Math.max(4, Math.round(width / 213)));
}

export function chunk<T>(items: T[], size: number): T[][] {
  const out: T[][] = [];
  for (let i = 0; i < items.length; i += size) out.push(items.slice(i, i + size));
  return out;
}

/**
 * First visible card of a row that scrolls sideways. The row moves only when focus would
 * reach the last visible card, so one more card always shows past the focus.
 */
export function rowOffset(offset: number, col: number, length: number, visible: number): number {
  const max = Math.max(0, length - visible);
  let next = offset;
  if (col < next) next = col;
  if (col > next + visible - 2) next = col - (visible - 2);
  return Math.min(max, Math.max(0, next));
}

export interface Shelves {
  focus: Focus;
  offsets: number[];
}

/**
 * Moves focus across rows that each scroll on their own (Netflix style). Up and down land
 * on the card at the same place on screen, not the same index.
 */
export function moveShelf(state: Shelves, dir: Direction, rows: number[], visible: (row: number) => number): Shelves {
  if (rows.length === 0) return state;
  const { row, col } = clamp(state.focus, rows);
  const offsets = rows.map((len, r) => Math.min(state.offsets[r] ?? 0, Math.max(0, len - 1)));
  let next: Focus = { row, col };

  if (dir === "left") next = { row, col: Math.max(0, col - 1) };
  if (dir === "right") next = { row, col: Math.min(rows[row] - 1, col + 1) };
  if ((dir === "up" && row > 0) || (dir === "down" && row < rows.length - 1)) {
    const target = dir === "up" ? row - 1 : row + 1;
    const onScreen = col - offsets[row];
    next = { row: target, col: Math.min(rows[target] - 1, offsets[target] + onScreen) };
  }
  offsets[next.row] = rowOffset(offsets[next.row], next.col, rows[next.row], visible(next.row));
  return { focus: next, offsets };
}
