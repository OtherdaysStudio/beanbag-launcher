import { useEffect, useMemo, useRef, useState, type CSSProperties, type MutableRefObject } from "react";
import { useArtPicker } from "../hooks/useArtPicker";
import { ART_KINDS, ART_LABEL, ORIGINAL, SOURCE_LABEL, matchLine } from "../lib/art";
import type { Device } from "../lib/input";
import { findOnline, type ArtKind, type Match } from "../lib/platform";
import type { Action, Game } from "../lib/types";
import { Hints, type Hint } from "./Chrome";

/** What App routes to this screen while it is open. */
export interface ArtControl {
  handle: (a: Action) => void;
  key: (e: KeyboardEvent) => boolean;
}

interface ArtScreenProps {
  game: Game;
  device: Device;
  control: MutableRefObject<ArtControl | null>;
  onChanged: () => void;
  onClose: () => void;
}

/** Tiles across the screen for each kind. */
const COLS: Record<ArtKind, number> = { cover: 8, background: 4, logo: 5 };

/** "Change art" on the big screen, like SteamGridDB's Steam Deck plugin: a tab for each
 *  kind, every image the game's sources have, and A applies one at once. */
export function ArtScreen({ game, device, control, onChanged, onClose }: ArtScreenProps) {
  const art = useArtPicker(game.id, onChanged);
  const [tab, setTab] = useState(0);
  const [at, setAt] = useState(0);
  const [gone, setGone] = useState<ReadonlySet<string>>(new Set());
  /** The Change game list: null while closed. */
  const [games, setGames] = useState<Match[] | "searching" | null>(null);
  const [gameAt, setGameAt] = useState(0);
  const body = useRef<HTMLDivElement>(null);

  const kind = ART_KINDS[tab];
  const cols = COLS[kind];
  const query = art.start?.query || game.name;
  const tiles = useMemo(
    () => [ORIGINAL, ...(art.choices?.[kind] ?? []).map((c) => c.url).filter((u) => !gone.has(u))],
    [art.choices, kind, gone],
  );
  const focus = Math.min(at, tiles.length - 1);
  const list = Array.isArray(games) ? games : [];

  const switchTab = (i: number) => {
    setTab(Math.max(0, Math.min(ART_KINDS.length - 1, i)));
    setAt(0);
  };
  const use = (url: string) => {
    if (art.saving === null) void art.pick(kind, url);
  };
  const openGames = () => {
    setGames("searching");
    setGameAt(0);
    findOnline(query)
      .then(setGames)
      .catch(() => setGames([]));
  };
  const showGame = (m: Match) => {
    setGames(null);
    setAt(0);
    art.show(m);
  };

  const handle = (a: Action) => {
    if (games !== null) {
      if (a === "b") setGames(null);
      else if (a === "up") setGameAt((i) => Math.max(0, i - 1));
      else if (a === "down") setGameAt((i) => Math.min(list.length - 1, i + 1));
      else if (a === "a" && list[gameAt]) showGame(list[gameAt]);
      return;
    }
    if (a === "b") onClose();
    else if (a === "lb" || a === "rb") switchTab(tab + (a === "rb" ? 1 : -1));
    else if (a === "left") setAt(Math.max(0, focus - 1));
    else if (a === "right") setAt(Math.min(tiles.length - 1, focus + 1));
    else if (a === "up") setAt(focus >= cols ? focus - cols : focus);
    else if (a === "down") setAt(Math.min(tiles.length - 1, focus + cols));
    else if (a === "a" && tiles[focus]) use(tiles[focus]);
    else if (a === "y") openGames();
  };
  const key = (e: KeyboardEvent) => {
    if (e.key !== "Tab") return false;
    handle(e.shiftKey ? "lb" : "rb");
    return true;
  };
  control.current = { handle, key };
  useEffect(
    () => () => {
      control.current = null;
    },
    [control],
  );

  // Keep the focused tile or game in view.
  useEffect(() => {
    body.current?.querySelector("[data-focused]")?.scrollIntoView({ block: "nearest" });
  }, [focus, tab, gameAt, games]);

  const pad = device === "pad";
  const hints: Hint[] =
    games !== null
      ? pad
        ? [["A", "Show images"], ["B", "Back"]]
        : [["Enter", "Show images"], ["Esc", "Back"]]
      : pad
        ? [["LB / RB", "Kind"], ["A", "Use"], ["Y", "Change game"], ["B", "Back"]]
        : [["Tab", "Kind"], ["Enter", "Use"], ["/", "Change game"], ["Esc", "Back"]];

  return (
    <section className="art" aria-label={`Change art for ${game.name}`}>
      <header className="art-head">
        <h1 className="art-title">Change art</h1>
        <p className="art-sub">
          {game.name}
          {art.match && ` · ${matchLine(game.name, art.match, art.choices)}`}
        </p>
        {games === null && (
          <nav className="art-tabs" role="tablist" aria-label="Kind of art">
            {ART_KINDS.map((k, i) => (
              <button key={k} type="button" role="tab" tabIndex={-1} aria-selected={i === tab} className="art-tab" onClick={() => switchTab(i)}>
                {ART_LABEL[k]}
                {art.choices && <span className="art-count">{art.choices[k].length}</span>}
              </button>
            ))}
          </nav>
        )}
      </header>

      <div className="art-body" ref={body}>
        {games !== null ? (
          <div className="art-games">
            {games === "searching" && <p className="art-state">Searching for “{query}”…</p>}
            {Array.isArray(games) && games.length === 0 && (
              <p className="art-state">No games match “{query}”. Search by another name in desktop mode.</p>
            )}
            {list.map((m, i) => (
              <button
                key={`${m.source}-${m.id}`}
                type="button"
                tabIndex={-1}
                className="art-game"
                data-focused={i === gameAt || undefined}
                onClick={() => showGame(m)}
              >
                {m.name}
                <small>
                  {SOURCE_LABEL[m.source]}
                  {m.source === "launchbox" && m.platform ? ` · ${m.platform}` : ""}
                </small>
              </button>
            ))}
          </div>
        ) : (
          <>
            {art.error && <p className="art-state art-error">{art.error}</p>}
            {art.loading && <p className="art-state">Finding images for {game.name}…</p>}
            {!art.loading && !art.match && <p className="art-state">No game matches “{query}”. Change game to search again.</p>}
            {art.choices && (
              <div className={`art-grid art-${kind}`} style={{ "--cols": cols } as CSSProperties}>
                {tiles.map((url, i) => {
                  const choice = art.choices?.[kind].find((c) => c.url === url);
                  const used = art.applied[kind] === url;
                  return (
                    <div key={url} className="art-cell">
                      <div
                        className="card art-tile"
                        role="button"
                        aria-label={choice ? `Use this ${ART_LABEL[kind].toLowerCase()} from ${choice.source}` : "Use the game's own art"}
                        data-focused={i === focus || undefined}
                        onClick={() => {
                          setAt(i);
                          use(url);
                        }}
                      >
                        {choice ? (
                          <img
                            src={choice.thumb}
                            alt=""
                            loading="lazy"
                            decoding="async"
                            draggable={false}
                            onError={() => setGone((g) => new Set(g).add(url))}
                          />
                        ) : (
                          <span className="art-original">Original</span>
                        )}
                      </div>
                      <span className="art-source" data-used={used || undefined}>
                        {art.saving === url ? "Saving…" : used ? "In use" : (choice?.source ?? "The game's own art")}
                      </span>
                    </div>
                  );
                })}
              </div>
            )}
            {art.choices?.note && <p className="art-state">{art.choices.note}</p>}
          </>
        )}
      </div>

      <Hints hints={hints} keys={!pad} />
    </section>
  );
}
