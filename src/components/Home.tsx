import { Fragment, memo, useState, type CSSProperties } from "react";
import { useGlide } from "../hooks/useGlide";
import { useLogoTone } from "../hooks/useLogoTone";
import type { Focus } from "../lib/nav";
import type { Game } from "../lib/types";
import { Poster, Tile } from "./Poster";

interface RowProps {
  games: Game[];
  row: number;
  /** -1 when focus is on another row. */
  focusedCol: number;
  wide?: boolean;
  onPick: (row: number, col: number) => void;
}

export const Row = memo(function Row({ games, row, focusedCol, wide, onPick }: RowProps) {
  const Card = wide ? Tile : Poster;
  return (
    <>
      {games.map((g, col) => (
        <Card key={g.id} game={g} row={row} col={col} focused={col === focusedCol} onPick={onPick} />
      ))}
    </>
  );
});

export function HeroInfo({ game }: { game?: Game }) {
  const [logoFailed, setLogoFailed] = useState(false);
  const tone = useLogoTone(game?.art.logo);
  if (!game) return <header className="hero-info" />;
  return (
    <header className="hero-info">
      {game.art.logo && !logoFailed ? (
        <img className="hero-logo" data-tone={tone} src={game.art.logo} alt={game.name} decoding="async" onError={() => setLogoFailed(true)} />
      ) : (
        <h1 className="hero-title">{game.name}</h1>
      )}
    </header>
  );
}

interface HomeProps {
  rows: Game[][];
  hasRecent: boolean;
  total: number;
  cols: number;
  focus: Focus;
  /** False while an overlay owns the controller: no focus ring. */
  active: boolean;
  onPick: (row: number, col: number) => void;
}

export function Home({ rows, hasRecent, total, cols, focus, active, onPick }: HomeProps) {
  const game = rows[focus.row]?.[focus.col];
  const { pageRef, setRow } = useGlide(focus.row, [rows]);

  const libraryStart = hasRecent ? 1 : 0;
  return (
    <div
      className="page home"
      ref={pageRef}
      data-scrolled={focus.row > 0 || undefined}
      style={{ "--cols": cols } as CSSProperties}
    >
      <HeroInfo key={game?.id} game={game} />
      {rows.map((games, r) => (
        <Fragment key={r}>
          {hasRecent && r === 0 && <h2 className="label">Recent</h2>}
          {r === libraryStart && (
            <h2 className="label">
              Library <span className="label-count">{total}</span>
            </h2>
          )}
          <div
            className={hasRecent && r === 0 ? "row tiles" : "row posters"}
            ref={setRow(r)}
          >
            <Row
              games={games}
              row={r}
              wide={hasRecent && r === 0}
              focusedCol={active && focus.row === r ? focus.col : -1}
              onPick={onPick}
            />
          </div>
        </Fragment>
      ))}
    </div>
  );
}
