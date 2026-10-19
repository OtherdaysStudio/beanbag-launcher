import type { CSSProperties } from "react";
import { useGlide } from "../hooks/useGlide";
import type { Focus } from "../lib/nav";
import type { Game } from "../lib/types";
import { Row } from "./Home";

/** One row of letters, like Apple TV: left and right to pick, A to type. */
export const KEYS = [..."abcdefghijklmnopqrstuvwxyz0123456789", " ", "⌫"];

interface SearchProps {
  open: boolean;
  query: string;
  keyIndex: number;
  area: "keys" | "results";
  rows: Game[][];
  focus: Focus;
  cols: number;
  onKeyPoint: (i: number) => void;
  onKeyPick: (i: number) => void;
  onPick: (row: number, col: number) => void;
}

export function Search(p: SearchProps) {
  const { pageRef, setRow } = useGlide(p.area === "results" ? p.focus.row : 0, [p.rows], 0.62);
  const count = p.rows.reduce((n, r) => n + r.length, 0);
  const label = p.query ? (count ? "Results" : "") : "Recently played";

  return (
    <section className="search" data-open={p.open || undefined} aria-hidden={!p.open}>
      <div className="search-head">
        <p className="search-query" data-empty={!p.query || undefined}>
          {p.query || "Search"}
          <span className="caret" />
        </p>
        <div className="keys" role="toolbar" aria-label="Letters">
          {KEYS.map((k, i) => (
            <span
              key={k}
              className={k === " " ? "key key-wide" : "key"}
              aria-label={k === " " ? "Space" : k === "⌫" ? "Delete" : k}
              data-focused={(p.open && p.area === "keys" && i === p.keyIndex) || undefined}
              onPointerMove={() => p.onKeyPoint(i)}
              onClick={() => p.onKeyPick(i)}
            >
              {k === " " ? "space" : k}
            </span>
          ))}
        </div>
      </div>
      <div className="search-results">
        <div className="page" ref={pageRef} style={{ "--cols": p.cols } as CSSProperties}>
          {label && <h2 className="label">{label}</h2>}
          {p.query && !count && <p className="empty">No games match “{p.query}”.</p>}
          {p.rows.map((games, r) => (
            <div className="row posters" key={r} ref={setRow(r)}>
              <Row
                games={games}
                row={r}
                focusedCol={p.open && p.area === "results" && p.focus.row === r ? p.focus.col : -1}
                onPick={p.onPick}
              />
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
