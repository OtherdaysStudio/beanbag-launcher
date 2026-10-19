import { memo, useState } from "react";
import { useLogoTone } from "../hooks/useLogoTone";
import type { Game } from "../lib/types";

export interface CardProps {
  game: Game;
  focused: boolean;
  row: number;
  col: number;
  /** Click: focus this card and open it. Hover only lifts it, so the page never moves under the cursor. */
  onPick: (row: number, col: number) => void;
}

/** Portrait cover. Focus lifts it and names it. */
export const Poster = memo(function Poster({ game, focused, row, col, onPick }: CardProps) {
  const [failed, setFailed] = useState(false);
  return (
    <div
      className="card poster"
      role="button"
      aria-label={game.name}
      data-focused={focused || undefined}
      onClick={() => onPick(row, col)}
    >
      {game.art.cover && !failed ? (
        <img
          src={game.art.cover}
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
          onError={() => setFailed(true)}
        />
      ) : (
        <div className="card-fallback">{game.name}</div>
      )}
      <div className="poster-info">
        <span className="poster-name">{game.name}</span>
      </div>
    </div>
  );
});

/** Wide tile for the Recent row: hero art cropped, logo on top. */
export const Tile = memo(function Tile({ game, focused, row, col, onPick }: CardProps) {
  const [artFailed, setArtFailed] = useState(false);
  const [logoFailed, setLogoFailed] = useState(false);
  const tone = useLogoTone(game.art.logo);
  const art = game.art.hero;
  return (
    <div
      className="card tile"
      role="button"
      aria-label={game.name}
      data-focused={focused || undefined}
      onClick={() => onPick(row, col)}
    >
      {art && !artFailed && (
        <>
          <img className="tile-art" src={art} alt="" decoding="async" draggable={false} onError={() => setArtFailed(true)} />
          <div className="tile-scrim" />
        </>
      )}
      {game.art.logo && !logoFailed ? (
        <img
          className="tile-logo"
          data-tone={tone}
          src={game.art.logo}
          alt=""
          decoding="async"
          draggable={false}
          onError={() => setLogoFailed(true)}
        />
      ) : (
        <span className="tile-name">{game.name}</span>
      )}
    </div>
  );
});
