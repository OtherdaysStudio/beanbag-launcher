import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ArtScreen, type ArtControl } from "./components/ArtScreen";
import { Backdrop } from "./components/Backdrop";
import { Clock, Hints, type Hint } from "./components/Chrome";
import { Home } from "./components/Home";
import { Menu, type MenuItem } from "./components/Menu";
import { KEYS, Search } from "./components/Search";
import { Sheet } from "./components/Sheet";
import { Shelves } from "./components/Shelves";
import { Desktop } from "./desktop/Desktop";
import { ScanDialog } from "./desktop/ScanDialog";
import { useLogoTone } from "./hooks/useLogoTone";
import { startInput, type Device } from "./lib/input";
import { cachedLibrary, loadLibrary } from "./lib/library";
import { chunk, clamp, columnsFor, move, moveShelf, rowOffset, type Focus } from "./lib/nav";
import {
  addGames,
  autostartEnabled,
  chooseFile,
  launchGame,
  on,
  quit,
  scanFolder,
  setAutostart,
  setView,
  showWindow,
  type Candidate,
} from "./lib/platform";
import { search } from "./lib/search";
import { buildShelves, SORTS, type Layout, type Sort } from "./lib/shelves";
import type { Action, Direction, Game, Library } from "./lib/types";

type Mode = "home" | "sheet" | "search" | "menu" | "launching" | "art";
type From = "home" | "search";

const PAGE_ROWS = 3;
const LAYOUT_KEY = "beanbag.layout";
const SORT_KEY = "beanbag.sort";

function saved<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  try {
    const v = localStorage.getItem(key) as T | null;
    return v && allowed.includes(v) ? v : fallback;
  } catch {
    return fallback;
  }
}

function remember(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Not saved: the next start uses the default.
  }
}

const sortLabel = (s: Sort) => SORTS.find(([k]) => k === s)?.[1] ?? "";
const DIRECTIONS: ReadonlySet<Action> = new Set(["up", "down", "left", "right"]);
const isDirection = (a: Action): a is Direction => DIRECTIONS.has(a);
const TYPEABLE = /^[\p{L}\p{N} '’:.&-]$/u;

type HintSet = Record<Mode | "results", Hint[]>;

const PAD_HINTS: HintSet = {
  home: [
    ["A", "Open"],
    ["Y", "Search"],
    ["≡", "Menu"],
  ],
  sheet: [
    ["A", "Play"],
    ["X", "Change art"],
    ["B", "Back"],
  ],
  search: [
    ["A", "Type"],
    ["X", "Delete"],
    ["B", "Back"],
  ],
  results: [
    ["A", "Open"],
    ["B", "Letters"],
  ],
  menu: [
    ["A", "Select"],
    ["B", "Close"],
  ],
  launching: [["B", "Back"]],
  // The art screen shows its own hints.
  art: [],
};

const KEY_HINTS: HintSet = {
  home: [
    ["Enter", "Open"],
    ["A–Z", "Search"],
    ["Esc", "Menu"],
  ],
  sheet: [
    ["Enter", "Play"],
    ["Esc", "Back"],
  ],
  search: [
    ["↓", "Results"],
    ["Esc", "Back"],
  ],
  results: [
    ["Enter", "Open"],
    ["Esc", "Letters"],
  ],
  menu: [
    ["Enter", "Select"],
    ["Esc", "Close"],
  ],
  launching: [["Esc", "Back"]],
  art: [],
};

function useColumns() {
  const [cols, setCols] = useState(() => columnsFor(window.innerWidth));
  useEffect(() => {
    const onResize = () => setCols(columnsFor(window.innerWidth));
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  return cols;
}

const byRecent = (a: Game, b: Game) => b.lastPlayed - a.lastPlayed;
const same = (r: number, c: number) => (f: Focus) => (f.row === r && f.col === c ? f : { row: r, col: c });

export function App() {
  const [library, setLibrary] = useState<Library | null>(cachedLibrary);
  const [mode, setMode] = useState<Mode>("home");
  const [focus, setFocus] = useState<Focus>({ row: 0, col: 0 });
  const [sheet, setSheet] = useState<{ game: Game; from: From } | null>(null);
  const [query, setQuery] = useState("");
  const [keyIndex, setKeyIndex] = useState(0);
  const [area, setArea] = useState<"keys" | "results">("keys");
  const [found, setFound] = useState<Focus>({ row: 0, col: 0 });
  const [menuIndex, setMenuIndex] = useState(0);
  const [autostart, setAutostartState] = useState<boolean | null>(null);
  const [launching, setLaunching] = useState<Game | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [device, setDevice] = useState<Device>("pad");
  const [layout, setLayout] = useState<Layout>(() => saved<Layout>(LAYOUT_KEY, ["rows", "grid"], "rows"));
  const [sort, setSort] = useState<Sort>(() => saved<Sort>(SORT_KEY, SORTS.map(([k]) => k), "recent"));
  const [offsets, setOffsets] = useState<number[]>([]);
  // The browser preview opens desktop mode with #desktop, for screenshots.
  const [view, setViewState] = useState<"big" | "desktop">(() => (location.hash === "#desktop" ? "desktop" : "big"));
  const [scan, setScan] = useState<{ folder: string; candidates: Candidate[] } | null>(null);
  const cols = useColumns();
  /** The game page's focused action: 0 Play, 1 Change art. */
  const [sheetFocus, setSheetFocus] = useState(0);
  const [artFor, setArtFor] = useState<Game | null>(null);
  /** Where Change art opened from, so Back returns there. */
  const [artFrom, setArtFrom] = useState<"sheet" | "home">("sheet");
  const artControl = useRef<ArtControl | null>(null);
  const launchTone = useLogoTone(launching?.art.logo);

  // Library → shelves (rows by catalog and genre, or the grid).
  const games = useMemo(() => (library?.games ?? []).filter((g) => !g.hidden), [library]);
  const recent = useMemo(() => games.filter((g) => g.lastPlayed > 0).sort(byRecent), [games]);
  const rowOrder = library?.rows;
  const shelves = useMemo(() => buildShelves(games, layout, cols, rowOrder ?? [], sort), [games, layout, cols, rowOrder, sort]);
  const lengths = useMemo(() => shelves.map((s) => s.games.length), [shelves]);
  const visibleIn = useCallback((r: number) => shelves[r]?.visible ?? cols, [shelves, cols]);
  const at = clamp(focus, lengths);
  const focused: Game | undefined = shelves[at.row]?.games[at.col];
  const sheetGame = sheet ? (library?.games.find((g) => g.id === sheet.game.id) ?? sheet.game) : null;

  // Search → rows.
  const resultRows = useMemo(
    () => chunk(query ? search(games, query) : recent.slice(0, cols), cols),
    [games, recent, query, cols],
  );
  const resultLengths = useMemo(() => resultRows.map((r) => r.length), [resultRows]);
  const foundAt = clamp(found, resultLengths);

  const rescan = useCallback(() => {
    loadLibrary()
      .then(setLibrary)
      .catch((e) => setNotice(`The library could not be read. ${String(e)}`));
  }, []);

  useEffect(() => {
    rescan();
    requestAnimationFrame(() => requestAnimationFrame(() => void showWindow()));
    autostartEnabled().then(setAutostartState).catch(() => setAutostartState(null));
  }, [rescan]);

  useEffect(() => {
    // Playtime and "Recent" changed: read the library again.
    const off = on("game-exited", () => {
      setLaunching(null);
      setMode("home");
      setFocus({ row: 0, col: 0 });
      setOffsets([]);
      rescan();
    });
    // Art arrived in the background: read the library again, keep focus where it is.
    const offArt = on("library-changed", rescan);
    return () => {
      void off.then((f) => f());
      void offArt.then((f) => f());
    };
  }, [rescan]);

  // A game that never reports back (failed launch, closed launcher) must not trap the screen.
  useEffect(() => {
    if (mode !== "launching") return;
    const t = setTimeout(() => {
      setLaunching(null);
      setMode("home");
    }, 130_000);
    return () => clearTimeout(t);
  }, [mode]);

  useEffect(() => {
    if (!notice) return;
    const t = setTimeout(() => setNotice(null), 5000);
    return () => clearTimeout(t);
  }, [notice]);

  // Hide the cursor until the mouse moves.
  useEffect(() => {
    let t: ReturnType<typeof setTimeout>;
    const wake = () => {
      document.body.classList.remove("idle");
      clearTimeout(t);
      t = setTimeout(() => document.body.classList.add("idle"), 2500);
    };
    document.body.classList.add("idle");
    window.addEventListener("pointermove", wake);
    return () => window.removeEventListener("pointermove", wake);
  }, []);

  // Like the Steam Deck: Start on a focused game offers Change art first.
  const menuItems: MenuItem[] = [
    ...(focused ? [{ label: "Change art", value: focused.name }] : []),
    { label: "Desktop mode" },
    { label: "Rescan library" },
    { label: "Library layout", value: layout === "rows" ? "Rows" : "Grid" },
    { label: "Sort rows", value: sortLabel(sort) },
    ...(autostart === null ? [] : [{ label: "Start with Windows", value: autostart ? "On" : "Off" }]),
    { label: "Quit to desktop" },
  ];

  // Actions.
  const openSheet = (game: Game, from: From) => {
    setSheet({ game, from });
    setSheetFocus(0);
    setMode("sheet");
  };
  const closeSheet = () => setMode(sheet?.from ?? "home");
  const openArt = (game: Game, from: "sheet" | "home" = "sheet") => {
    setArtFor(game);
    setArtFrom(from);
    setMode("art");
  };
  const closeArt = () => {
    setArtFor(null);
    setMode(artFrom);
  };
  const play = (game: Game) => {
    setLaunching(game);
    setMode("launching");
    launchGame(game.id).catch(() => {
      setLaunching(null);
      setMode("home");
      setNotice(`${game.name} did not start. Its files may have moved.`);
    });
  };
  const typeKey = (i: number) => {
    const k = KEYS[i];
    if (k === "⌫") setQuery((q) => q.slice(0, -1));
    else setQuery((q) => (q || k !== " " ? (q + k).slice(0, 40) : q));
  };
  const pickMenu = (i: number) => {
    const item = menuItems[i];
    if (!item) return;
    if (item.label === "Change art") {
      if (focused) openArt(focused, "home");
    } else if (item.label === "Desktop mode") {
      switchView("desktop");
    } else if (item.label === "Rescan library") {
      rescan();
      setMode("home");
      setNotice("Library rescanned.");
    } else if (item.label === "Library layout") {
      const next: Layout = layout === "rows" ? "grid" : "rows";
      setLayout(next);
      setFocus({ row: 0, col: 0 });
      setOffsets([]);
      remember(LAYOUT_KEY, next);
    } else if (item.label === "Sort rows") {
      nextSort();
    } else if (item.label === "Start with Windows") {
      const next = !autostart;
      setAutostartState(next);
      setAutostart(next).catch(() => setAutostartState(!next));
    } else {
      void quit();
    }
  };

  /** Cycles the sort. Rows start again at their first card; focus stays on its row. */
  const nextSort = () => {
    const i = SORTS.findIndex(([k]) => k === sort);
    const next = SORTS[(i + 1) % SORTS.length][0];
    setSort(next);
    remember(SORT_KEY, next);
    setFocus((f) => ({ row: f.row, col: 0 }));
    setOffsets([]);
  };

  const switchView = (next: "big" | "desktop") => {
    setViewState(next);
    setMode("home");
    void setView(next === "desktop");
  };
  const addFromFile = async () => {
    const exe = await chooseFile({ title: "Choose the game's program", extensions: ["exe", "lnk", "bat", "url"] });
    if (!exe) return;
    const added = await addGames([exe]);
    setNotice(added.length ? "Game added. Beanbag is finding its art." : "That game is already in the library.");
    rescan();
  };
  const addFromFolder = async () => {
    const folder = await chooseFile({ title: "Choose a folder of games", directory: true });
    if (folder) setScan({ folder, candidates: await scanFolder(folder) });
  };

  const handle = (a: Action) => {
    // Desktop mode is mouse and keyboard; the controller map belongs to big screen.
    if (view === "desktop") return;
    switch (mode) {
      case "home":
        if (isDirection(a) || a === "lb" || a === "rb") {
          const steps: Direction[] = isDirection(a) ? [a] : Array(PAGE_ROWS).fill(a === "lb" ? "up" : "down");
          if (layout === "rows") {
            let s = { focus: at, offsets };
            for (const d of steps) s = moveShelf(s, d, lengths, visibleIn);
            setFocus(s.focus);
            setOffsets(s.offsets);
          } else {
            setFocus(steps.reduce((f, d) => move(f, d, lengths), at));
          }
        } else if (a === "a") {
          if (focused) openSheet(focused, "home");
          else rescan();
        } else if (a === "b") {
          setFocus({ row: 0, col: 0 });
          setOffsets([]);
        }
        else if (a === "y") setMode("search");
        else if (a === "x") nextSort();
        else if (a === "menu") {
          setMenuIndex(0);
          setMode("menu");
        }
        return;

      case "sheet":
        if (a === "left" || a === "right") setSheetFocus(a === "right" ? 1 : 0);
        else if (a === "a" && sheetGame) (sheetFocus === 1 ? openArt : play)(sheetGame);
        else if (a === "x" && sheetGame) openArt(sheetGame);
        else if (a === "b") closeSheet();
        return;

      case "art":
        artControl.current?.handle(a);
        return;

      case "search":
        if (a === "y" || (a === "b" && area === "keys")) setMode("home");
        else if (a === "b") setArea("keys");
        else if (a === "x") setQuery((q) => q.slice(0, -1));
        else if (area === "keys") {
          if (a === "left") setKeyIndex((i) => Math.max(0, i - 1));
          else if (a === "right") setKeyIndex((i) => Math.min(KEYS.length - 1, i + 1));
          else if (a === "down" && resultRows.length) {
            setArea("results");
            setFound({ row: 0, col: 0 });
          } else if (a === "a") typeKey(keyIndex);
        } else if (a === "up" && foundAt.row === 0) setArea("keys");
        else if (isDirection(a)) setFound(move(foundAt, a, resultLengths));
        else if (a === "a") {
          const g = resultRows[foundAt.row]?.[foundAt.col];
          if (g) openSheet(g, "search");
        }
        return;

      case "menu":
        if (a === "up") setMenuIndex((i) => Math.max(0, i - 1));
        else if (a === "down") setMenuIndex((i) => Math.min(menuItems.length - 1, i + 1));
        else if (a === "a") pickMenu(menuIndex);
        else if (a === "b" || a === "menu") setMode("home");
        return;

      case "launching":
        if (a === "b") {
          setLaunching(null);
          setMode("home");
        }
        return;
    }
  };

  // Keyboard shortcuts: Esc opens the menu from home, and typing a letter starts a search.
  const onKey = (e: KeyboardEvent): boolean => {
    if (view === "desktop" || e.ctrlKey || e.metaKey || e.altKey) return false;
    if (mode === "art") return artControl.current?.key(e) ?? false;
    if (mode === "home" && e.key === "Escape") {
      setMenuIndex(0);
      setMode("menu");
      return true;
    }
    if (mode === "home" && e.key !== " " && TYPEABLE.test(e.key)) {
      setQuery(e.key);
      setArea("keys");
      setMode("search");
      return true;
    }
    if (mode !== "search") return false;
    if (e.key === "Backspace") {
      setQuery((q) => q.slice(0, -1));
      return true;
    }
    if (TYPEABLE.test(e.key) && (area === "keys" || e.key !== " ")) {
      setQuery((q) => (q || e.key !== " " ? (q + e.key).slice(0, 40) : q));
      setArea("keys");
      return true;
    }
    return false;
  };

  const openAt = (r: number, c: number) => {
    const g = shelves[r]?.games[c];
    if (!g) return;
    setFocus(same(r, c));
    setOffsets((o) => {
      const next = [...o];
      next[r] = rowOffset(o[r] ?? 0, c, lengths[r], visibleIn(r));
      return next;
    });
    openSheet(g, "home");
  };
  const openFoundAt = (r: number, c: number) => {
    const g = resultRows[r]?.[c];
    if (!g) return;
    setFound(same(r, c));
    openSheet(g, "search");
  };

  const handler = useRef({ handle, onKey, openAt, openFoundAt });
  handler.current = { handle, onKey, openAt, openFoundAt };
  useEffect(
    () =>
      startInput(
        (a) => handler.current.handle(a),
        (e) => handler.current.onKey(e),
        setDevice,
      ),
    [],
  );

  // Pointer: a click focuses and opens. Hover never moves focus, so the page stays still under the cursor.
  const pickHome = useCallback((r: number, c: number) => handler.current.openAt(r, c), []);
  const pickFound = useCallback((r: number, c: number) => handler.current.openFoundAt(r, c), []);

  const backdropGame =
    mode === "launching" ? launching : mode === "art" ? artFor : mode === "sheet" && sheet?.from === "search" ? sheetGame : focused;
  const dim = mode === "search" || mode === "menu" || mode === "art" || (mode === "home" && at.row > 0);
  const modeHints = (device === "pad" ? PAD_HINTS : KEY_HINTS)[mode === "search" && area === "results" ? "results" : mode];
  // On the game page, A and Enter name the focused action.
  const baseHints: Hint[] =
    mode === "sheet" && sheetFocus === 1 ? modeHints.map(([g, l]) => [g, g === "A" || g === "Enter" ? "Change art" : l]) : modeHints;
  // The controller shows the sort on X; the keyboard sorts from the menu.
  const hints: Hint[] = mode === "home" && device === "pad" ? [["X", sortLabel(sort)], ...baseHints] : baseHints;

  if (view === "desktop") {
    return (
      <>
        <Desktop
          games={library?.games ?? []}
          onBigScreen={() => switchView("big")}
          onAddGame={() => void addFromFile()}
          onScanFolder={() => void addFromFolder()}
          onPlay={play}
          onChanged={rescan}
        />
        {scan && (
          <ScanDialog
            folder={scan.folder}
            candidates={scan.candidates}
            onCancel={() => setScan(null)}
            onAdd={(exes) => {
              setScan(null);
              void addGames(exes).then((added) => {
                setNotice(`${added.length} ${added.length === 1 ? "game" : "games"} added. Beanbag is finding their art.`);
                rescan();
              });
            }}
          />
        )}
        {notice && (
          <p className="notice" role="status">
            {notice}
          </p>
        )}
      </>
    );
  }

  return (
    <main className="app" data-mode={mode} data-from={sheet?.from}>
      <Backdrop src={backdropGame?.art.hero} className={dim ? "dim" : ""} />

      {library === null ? (
        <p className="state">Reading your library</p>
      ) : !library.found ? (
        <div className="state">
          <h1>No games yet</h1>
          <p>Install Steam or import your Playnite library, then press A to rescan.</p>
        </div>
      ) : games.length === 0 ? (
        <div className="state">
          <h1>No installed games</h1>
          <p>Install a game, then press A to rescan.</p>
        </div>
      ) : (
        layout === "rows" ? (
          <Shelves shelves={shelves} focus={at} offsets={offsets} active={mode === "home"} cols={cols} onPick={pickHome} />
        ) : (
          <Home
            rows={shelves.map((s) => s.games)}
            hasRecent={shelves[0]?.wide === true}
            total={games.length}
            cols={cols}
            focus={at}
            active={mode === "home"}
            onPick={pickHome}
          />
        )
      )}

      <Search
        open={mode === "search" || (mode === "sheet" && sheet?.from === "search")}
        query={query}
        keyIndex={keyIndex}
        area={area}
        rows={resultRows}
        focus={foundAt}
        cols={cols}
        onKeyPoint={(i) => {
          setArea("keys");
          setKeyIndex(i);
        }}
        onKeyPick={typeKey}
        onPick={pickFound}
      />

      <Sheet
        game={sheetGame}
        open={mode === "sheet"}
        focus={sheetFocus}
        onPlay={() => sheetGame && play(sheetGame)}
        onChangeArt={() => sheetGame && openArt(sheetGame)}
        onClose={closeSheet}
      />

      <Menu
        open={mode === "menu"}
        items={menuItems}
        index={menuIndex}
        onPoint={setMenuIndex}
        onPick={pickMenu}
        onClose={() => setMode("home")}
      />

      <section className="launch" data-open={mode === "launching" || undefined} aria-hidden={mode !== "launching"}>
        {launching && (
          <>
            {launching.art.hero && <img className="launch-art" src={launching.art.hero} alt="" />}
            {launching.art.logo ? (
              <img className="launch-logo" data-tone={launchTone} src={launching.art.logo} alt={launching.name} />
            ) : (
              <h1 className="hero-title">{launching.name}</h1>
            )}
            <p className="launch-status">Starting</p>
            <div className="launch-bar" />
          </>
        )}
      </section>

      {mode === "art" && artFor && (
        <ArtScreen game={artFor} device={device} control={artControl} onChanged={rescan} onClose={closeArt} />
      )}

      <Clock />
      {mode !== "art" && <Hints hints={hints} keys={device === "keys"} />}
      {notice && (
        <p className="notice" role="status">
          {notice}
        </p>
      )}
    </main>
  );
}
