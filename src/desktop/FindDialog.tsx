import { useEffect, useRef, useState, type ReactNode } from "react";
import { useArtPicker, type ArtPicker } from "../hooks/useArtPicker";
import { ART_KINDS, ART_LABEL, ORIGINAL, SOURCE_LABEL, matchLine } from "../lib/art";
import { chooseFile, findOnline, setSteamGridDbKey, type ArtKind, type Match } from "../lib/platform";
import type { Game } from "../lib/types";

interface FindDialogProps {
  game: Game;
  onChanged: () => void;
  onClose: () => void;
}

const IMAGES = ["png", "jpg", "jpeg", "webp"];
const nowArt = (game: Game, kind: ArtKind) => (kind === "cover" ? game.art.cover : kind === "background" ? game.art.hero : game.art.logo);

/** "Change art" in desktop mode: a tab for each kind of art, every image the game's sources
 *  have, and a click applies one at once. */
export function FindDialog({ game, onChanged, onClose }: FindDialogProps) {
  const art = useArtPicker(game.id, onChanged);
  const [tab, setTab] = useState<ArtKind>("cover");
  const [changing, setChanging] = useState(false);

  const fromFile = async () => {
    const path = await chooseFile({ title: `Choose a ${ART_LABEL[tab].toLowerCase()}`, extensions: IMAGES });
    if (path) void art.pick(tab, path);
  };

  return (
    <div className="dt-dialog" role="dialog" aria-modal="true" aria-labelledby="art-title" onKeyDown={(e) => e.key === "Escape" && onClose()}>
      <div className="dt-dialog-panel dt-art-panel">
        <header className="dt-dialog-head dt-art-head">
          <div>
            <h2 id="art-title">Change art for {game.name}</h2>
            {!changing && <p>{art.loading && !art.match ? "Finding the game online…" : art.match ? matchLine(game.name, art.match, art.choices) : "No match found yet"}</p>}
          </div>
          <button type="button" className="dt-button" onClick={() => setChanging((c) => !c)}>
            {changing ? "Back to images" : "Change game…"}
          </button>
        </header>

        {changing ? (
          <ChangeGame
            query={art.start?.query || game.name}
            onPick={(m) => {
              setChanging(false);
              art.show(m);
            }}
          />
        ) : (
          <>
            <div className="dt-tabs" role="tablist" aria-label="Kind of art">
              {ART_KINDS.map((k) => (
                <button key={k} type="button" role="tab" aria-selected={tab === k} className="dt-tab" onClick={() => setTab(k)}>
                  {ART_LABEL[k]}
                  {art.choices && <span className="dt-tab-count">{art.choices[k].length}</span>}
                </button>
              ))}
              <button type="button" className="dt-button dt-tabs-file" onClick={() => void fromFile()}>
                Choose a file…
              </button>
            </div>

            <div className="dt-dialog-list" aria-busy={art.loading}>
              {art.error && <p className="dt-wait dt-error">{art.error}</p>}
              {art.loading && <p className="dt-wait">Finding images on Steam, LaunchBox and SteamGridDB…</p>}
              {!art.loading && !art.match && (
                <p className="dt-wait">
                  No game matches “{art.start?.query || game.name}”. Choose Change game to search by another name.
                </p>
              )}
              {art.choices && (
                <div className={`dt-choices dt-choices-${tab}`} role="list">
                  <div className="dt-choice dt-choice-now" role="listitem">
                    <span className="dt-choice-art">{nowArt(game, tab) ? <img src={nowArt(game, tab)} alt="" /> : <span className="dt-missing">None</span>}</span>
                    <span className="dt-choice-source">In use now</span>
                  </div>
                  <Tile art={art} kind={tab} url={ORIGINAL} source="The game's own art">
                    <span className="dt-choice-word">Original</span>
                  </Tile>
                  {art.choices[tab].map((c) => (
                    <Tile key={c.url} art={art} kind={tab} url={c.url} source={c.source} thumb={c.thumb} />
                  ))}
                  {art.choices[tab].length === 0 && <p className="dt-wait">No {ART_LABEL[tab].toLowerCase()} images from this game's sources.</p>}
                </div>
              )}
              {art.choices?.note && <p className="dt-wait">{art.choices.note}</p>}
            </div>

            {art.choices && (art.choices.description || art.choices.genres.length > 0) && (
              <div className="dt-details">
                <div className="dt-details-text">
                  <strong>Description and genres from {art.match && SOURCE_LABEL[art.match.source]}</strong>
                  <p>{[art.choices.genres.join(", "), art.choices.description].filter(Boolean).join(" · ")}</p>
                </div>
                <button type="button" className="dt-button" disabled={art.detailsUsed} onClick={() => void art.useDetails()}>
                  {art.detailsUsed ? "Used" : "Use"}
                </button>
              </div>
            )}
          </>
        )}

        <footer className="dt-dialog-foot dt-art-foot">
          <KeyField
            on={art.start?.steamGridDb ?? false}
            onSaved={() => {
              if (art.match) art.show(art.match);
            }}
          />
          <button type="button" className="dt-button dt-button-primary" onClick={onClose}>
            Done
          </button>
        </footer>
      </div>
    </div>
  );
}

interface TileProps {
  art: ArtPicker;
  kind: ArtKind;
  url: string;
  source: string;
  /** The thumbnail; none for the Original tile, which shows its word instead. */
  thumb?: string;
  children?: ReactNode;
}

/** One image to pick. A click applies it at once; the tile then says so. A tile whose
 *  thumbnail is gone (a 404) leaves the grid. */
function Tile({ art, kind, url, source, thumb, children }: TileProps) {
  const [gone, setGone] = useState(false);
  if (gone) return null;
  const used = art.applied[kind] === url;
  return (
    <button
      type="button"
      role="listitem"
      className="dt-choice"
      aria-pressed={used}
      aria-label={url === ORIGINAL ? "Use the game's own art" : `Use this image from ${source}`}
      disabled={art.saving !== null}
      onClick={() => void art.pick(kind, url)}
    >
      <span className="dt-choice-art">{thumb ? <img src={thumb} alt="" loading="lazy" onError={() => setGone(true)} /> : children}</span>
      <span className="dt-choice-source">{art.saving === url ? "Saving…" : used ? "Applied" : source}</span>
    </button>
  );
}

/** Search by another name and pick the right game. */
function ChangeGame({ query: first, onPick }: { query: string; onPick: (m: Match) => void }) {
  const [query, setQuery] = useState(first);
  const [results, setResults] = useState<Match[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);

  const run = (q: string) => {
    if (!q.trim()) return;
    setBusy(true);
    setError(null);
    findOnline(q.trim())
      .then(setResults)
      .catch(() => setError("The search did not finish. Check the internet connection."))
      .finally(() => setBusy(false));
  };

  useEffect(() => {
    input.current?.select();
    run(first);
    // Search once for the game's own name when the view opens.
  }, []);

  return (
    <>
      <form
        className="dt-find-form dt-change-form"
        onSubmit={(e) => {
          e.preventDefault();
          run(query);
        }}
      >
        <input ref={input} className="dt-search" value={query} onChange={(e) => setQuery(e.target.value)} aria-label="Game name" />
        <button type="submit" className="dt-button" disabled={busy}>
          Search
        </button>
      </form>
      <div className="dt-dialog-list" aria-busy={busy}>
        {busy && <p className="dt-wait">Searching. The first search also downloads the LaunchBox database (about 100 MB).</p>}
        {error && <p className="dt-wait dt-error">{error}</p>}
        {!busy && results?.length === 0 && <p className="dt-wait">No games match “{query}”. Try a shorter name.</p>}
        {!busy &&
          results?.map((m) => (
            <button key={`${m.source}-${m.id}`} type="button" className="dt-result" onClick={() => onPick(m)}>
              <span className="dt-result-art">{m.image && <img src={m.image} alt="" loading="lazy" />}</span>
              <span className="dt-result-name">
                {m.name}
                <small>
                  {SOURCE_LABEL[m.source]}
                  {m.source === "launchbox" && m.platform ? ` · ${m.platform}` : ""}
                </small>
              </span>
              <span className="dt-result-use">Show images</span>
            </button>
          ))}
      </div>
    </>
  );
}

/** More images with a free SteamGridDB key. The key stays on this PC. */
function KeyField({ on: initial, onSaved }: { on: boolean; onSaved: () => void }) {
  const [on, setOn] = useState(initial);
  const [open, setOpen] = useState(false);
  const [key, setKey] = useState("");
  const [state, setState] = useState<"idle" | "saving" | "error">("idle");
  const [message, setMessage] = useState("");
  useEffect(() => setOn(initial), [initial]);

  const save = (value: string) => {
    setState("saving");
    setSteamGridDbKey(value)
      .then(() => {
        setOn(value.trim() !== "");
        setOpen(false);
        setKey("");
        setState("idle");
        onSaved();
      })
      .catch((e) => {
        setState("error");
        setMessage(String(e));
      });
  };

  if (on) {
    return (
      <p className="dt-key">
        SteamGridDB is on.{" "}
        <button type="button" className="dt-link" onClick={() => save("")}>
          Remove key
        </button>
      </p>
    );
  }
  if (!open) {
    return (
      <p className="dt-key">
        <button type="button" className="dt-link" onClick={() => setOpen(true)}>
          Add a SteamGridDB key for more images
        </button>
      </p>
    );
  }
  return (
    <form
      className="dt-key dt-key-form"
      onSubmit={(e) => {
        e.preventDefault();
        save(key);
      }}
    >
      <input className="dt-search" value={key} onChange={(e) => setKey(e.target.value)} placeholder="API key from steamgriddb.com" aria-label="SteamGridDB API key" autoFocus />
      <button type="submit" className="dt-button" disabled={!key.trim() || state === "saving"}>
        {state === "saving" ? "Checking…" : "Save"}
      </button>
      {state === "error" && <span className="dt-error">{message}</span>}
    </form>
  );
}
