import { useEffect, useMemo, useRef, useState } from "react";
import { lastPlayed, playtime } from "../lib/format";
import { byName, search } from "../lib/search";
import type { Game } from "../lib/types";
import "./desktop.css";
import { Inspector } from "./Inspector";

/** What the list shows: everything, a smart list, a catalog or a genre. */
type Filter =
  | { kind: "all" }
  | { kind: "recent" }
  | { kind: "missing" }
  | { kind: "hidden" }
  | { kind: "catalog"; name: string }
  | { kind: "genre"; name: string };

const same = (a: Filter, b: Filter) =>
  a.kind === b.kind && ("name" in a ? a.name : "") === ("name" in b ? b.name : "");

type SortKey = "name" | "lastPlayed" | "playtime";

interface DesktopProps {
  games: Game[];
  onBigScreen: () => void;
  onAddGame: () => void;
  onScanFolder: () => void;
  onPlay: (game: Game) => void;
  onChanged: () => void;
}

function counts(games: Game[], pick: (g: Game) => string[]): Array<[string, number]> {
  const m = new Map<string, number>();
  for (const g of games) for (const k of pick(g)) m.set(k, (m.get(k) ?? 0) + 1);
  return [...m].sort(([a], [b]) => a.localeCompare(b));
}

const missingArt = (g: Game) => !g.art.cover || !g.art.hero || !g.art.logo;

/** Desktop mode: the library manager, for a mouse and keyboard at the desk. */
export function Desktop({ games, onBigScreen, onAddGame, onScanFolder, onPlay, onChanged }: DesktopProps) {
  const [filter, setFilter] = useState<Filter>({ kind: "all" });
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<{ key: SortKey; desc: boolean }>({ key: "name", desc: false });
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const visible = useMemo(() => games.filter((g) => !g.hidden), [games]);
  const catalogs = useMemo(() => counts(visible, (g) => g.catalogs), [visible]);
  const genres = useMemo(() => counts(visible, (g) => g.genres), [visible]);

  const list = useMemo(() => {
    const base =
      filter.kind === "hidden"
        ? games.filter((g) => g.hidden)
        : visible.filter((g) => {
            switch (filter.kind) {
              case "recent":
                return g.lastPlayed > 0;
              case "missing":
                return missingArt(g);
              case "catalog":
                return g.catalogs.includes(filter.name);
              case "genre":
                return g.genres.includes(filter.name);
              default:
                return true;
            }
          });
    if (query.trim()) return search(base, query);
    const dir = sort.desc ? -1 : 1;
    const by: Record<SortKey, (a: Game, b: Game) => number> = {
      name: byName,
      lastPlayed: (a, b) => a.lastPlayed - b.lastPlayed || byName(a, b),
      playtime: (a, b) => a.playtime - b.playtime || byName(a, b),
    };
    const sorted = [...base].sort(by[sort.key]);
    return dir === 1 ? sorted : sorted.reverse();
  }, [games, visible, filter, query, sort]);

  const selected = games.find((g) => g.id === selectedId) ?? list[0];

  // ⌘F / Ctrl+F focuses search, like every desktop app.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const nav = (label: string, f: Filter, count?: number) => (
    <button
      key={label}
      type="button"
      className="dt-nav"
      data-active={same(filter, f) || undefined}
      onClick={() => {
        setFilter(f);
        setSelectedId(null);
      }}
    >
      <span className="dt-nav-label">{label}</span>
      {count !== undefined && <span className="dt-nav-count">{count}</span>}
    </button>
  );

  const header = (label: string, key: SortKey) => (
    <button
      type="button"
      className="dt-th"
      data-sorted={sort.key === key ? (sort.desc ? "desc" : "asc") : undefined}
      onClick={() => setSort((s) => ({ key, desc: s.key === key ? !s.desc : key !== "name" }))}
    >
      {label}
    </button>
  );

  return (
    <div className="desktop">
      <aside className="dt-sidebar">
        <p className="dt-section">Library</p>
        {nav("All games", { kind: "all" }, visible.length)}
        {nav("Recently played", { kind: "recent" }, visible.filter((g) => g.lastPlayed > 0).length)}
        {nav("Missing art", { kind: "missing" }, visible.filter(missingArt).length)}
        {nav("Hidden", { kind: "hidden" }, games.length - visible.length)}
        <p className="dt-section">Catalogs</p>
        {catalogs.length ? catalogs.map(([name, n]) => nav(name, { kind: "catalog", name }, n)) : <p className="dt-hint">Add a game to a catalog in its details.</p>}
        <p className="dt-section">Genres</p>
        {genres.map(([name, n]) => nav(name, { kind: "genre", name }, n))}
      </aside>

      <section className="dt-main">
        <header className="dt-toolbar">
          <input
            ref={searchRef}
            className="dt-search"
            type="search"
            placeholder="Search"
            aria-label="Search games"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <div className="dt-actions">
            <button type="button" className="dt-button" onClick={onScanFolder}>
              Scan folder…
            </button>
            <button type="button" className="dt-button" onClick={onAddGame}>
              Add game…
            </button>
            <button type="button" className="dt-button dt-button-primary" onClick={onBigScreen}>
              Big screen
            </button>
          </div>
        </header>

        <div className="dt-table" role="grid" aria-rowcount={list.length}>
          <div className="dt-row dt-head" role="row">
            <span />
            {header("Name", "name")}
            <span className="dt-th">Genres</span>
            {header("Last played", "lastPlayed")}
            {header("Playtime", "playtime")}
          </div>
          <div className="dt-body">
            {list.map((g) => (
              <div
                key={g.id}
                role="row"
                className="dt-row"
                data-selected={g.id === selected?.id || undefined}
                onClick={() => setSelectedId(g.id)}
                onDoubleClick={() => onPlay(g)}
              >
                <span className="dt-thumb">{g.art.cover && <img src={g.art.cover} alt="" loading="lazy" decoding="async" />}</span>
                <span className="dt-name">
                  {g.name}
                  <span className="dt-sub">{[g.platform && !g.platform.startsWith("PC") ? g.platform : g.source].filter(Boolean).join(" · ")}</span>
                </span>
                <span className="dt-cell">{g.genres.join(", ")}</span>
                <span className="dt-cell">{g.lastPlayed ? lastPlayed(g.lastPlayed) : ""}</span>
                <span className="dt-cell dt-num">{playtime(g.playtime)}</span>
              </div>
            ))}
            {!list.length && <p className="dt-empty">{query ? `No games match “${query}”.` : "Nothing here yet."}</p>}
          </div>
        </div>
      </section>

      {selected && <Inspector key={selected.id} game={selected} allGenres={genres.map(([g]) => g)} allCatalogs={catalogs.map(([c]) => c)} onPlay={onPlay} onChanged={onChanged} />}
    </div>
  );
}
