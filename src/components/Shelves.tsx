import { memo, type CSSProperties } from "react";
import { useGlide } from "../hooks/useGlide";
import type { Focus } from "../lib/nav";
import type { Game } from "../lib/types";
import { HeroInfo } from "./Home";
import { Poster, Tile } from "./Poster";

export interface Shelf {
  key: string;
  title: string;
  games: Game[];
  /** Landscape tiles instead of posters. */
  wide?: boolean;
  /** Cards across the screen. */
  visible?: number;
}

/** Cards each side of the visible ones, so a row never shows a gap while it slides. */
const SPARE = 3;

interface TrackProps {
  games: Game[];
  row: number;
  wide?: boolean;
  offset: number;
  visible: number;
  focusedCol: number;
  onPick: (row: number, col: number) => void;
}

/** One sideways row. Only the cards near the visible window are in the page. */
const Track = memo(function Track({ games, row, wide, offset, visible, focusedCol, onPick }: TrackProps) {
  const Card = wide ? Tile : Poster;
  const from = Math.max(0, offset - SPARE);
  const to = Math.min(games.length, offset + visible + SPARE);
  return (
    <div
      className={wide ? "track tiles" : "track posters"}
      style={{ "--per": visible, transform: `translate3d(calc(${-offset} * var(--step)), 0, 0)` } as CSSProperties}
    >
      {games.slice(from, to).map((g, i) => {
        const col = from + i;
        return (
          <div className="slot" key={g.id} style={{ left: `calc(${col} * var(--step))` }}>
            <Card game={g} row={row} col={col} focused={col === focusedCol} onPick={onPick} />
          </div>
        );
      })}
    </div>
  );
});

interface ShelvesProps {
  shelves: Shelf[];
  focus: Focus;
  offsets: number[];
  active: boolean;
  cols: number;
  onPick: (row: number, col: number) => void;
}

/** Netflix-style rows: genres and catalogs, each scrolling sideways on its own. */
export function Shelves({ shelves, focus, offsets, active, cols, onPick }: ShelvesProps) {
  const game = shelves[focus.row]?.games[focus.col];
  const { pageRef, setRow } = useGlide(focus.row, [shelves]);
  return (
    <div
      className="page home"
      ref={pageRef}
      data-scrolled={focus.row > 0 || undefined}
      style={{ "--cols": cols } as CSSProperties}
    >
      <HeroInfo key={game?.id} game={game} />
      {shelves.map((s, r) => (
        <section className="shelf" key={s.key} ref={setRow(r)} aria-label={s.title}>
          <h2 className="label">
            {s.title}
            {!s.wide && <span className="label-count">{s.games.length}</span>}
          </h2>
          <Track
            games={s.games}
            row={r}
            wide={s.wide}
            offset={offsets[r] ?? 0}
            visible={s.visible ?? cols}
            focusedCol={active && focus.row === r ? focus.col : -1}
            onPick={onPick}
          />
        </section>
      ))}
    </div>
  );
}
