import { size } from "../lib/format";
import type { Game } from "../lib/types";

interface SheetProps {
  game: Game | null;
  open: boolean;
  /** The focused action: 0 Play, 1 Change art. */
  focus: number;
  onPlay: () => void;
  onChangeArt: () => void;
  onClose: () => void;
}

/** "Steam", or the console for an emulated game. */
export function origin(game: Game): string {
  return game.platform && !game.platform.startsWith("PC") ? game.platform : game.source;
}

/** The game panel that slides up from the bottom. */
export function Sheet({ game, open, focus, onPlay, onChangeArt, onClose }: SheetProps) {
  const meta = game
    ? [origin(game), size(game.sizeBytes)]
        .filter(Boolean)
        .join("  ·  ")
    : "";
  return (
    <>
      {/* The game's art fills the screen behind the panel, at half brightness. */}
      <div className="stage" data-open={open || undefined} onClick={onClose} aria-hidden>
        {game?.art.hero && <img key={game.id} className="stage-art" src={game.art.hero} alt="" decoding="async" draggable={false} />}
      </div>
      <section className="sheet" data-open={open || undefined} aria-hidden={!open} aria-label={game?.name}>
        {game && (
          <>
            <div className="sheet-cover">
              {game.art.cover ? (
                <img src={game.art.cover} alt="" decoding="async" draggable={false} />
              ) : (
                <div className="card-fallback">{game.name}</div>
              )}
            </div>
            <div className="sheet-body">
              <h2 className="sheet-title">{game.name}</h2>
              <p className="sheet-meta">{meta}</p>
              {game.description && <p className="sheet-description">{game.description}</p>}
              <div className="sheet-actions">
                <button type="button" tabIndex={-1} className="pill pill-primary" data-focused={(open && focus === 0) || undefined} onClick={onPlay}>
                  Play
                </button>
                <button type="button" tabIndex={-1} className="pill pill-quiet" data-focused={(open && focus === 1) || undefined} onClick={onChangeArt}>
                  Change art
                </button>
              </div>
            </div>
          </>
        )}
      </section>
    </>
  );
}
