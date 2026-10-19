import { isTauri } from "./library";

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T | undefined> {
  if (!isTauri) {
    console.info(`[preview] ${cmd}`, args ?? "");
    return undefined;
  }
  const core = await import("@tauri-apps/api/core");
  return core.invoke<T>(cmd, args);
}

export const launchGame = (id: string) => invoke<void>("launch_game", { id });
export const quit = () => invoke<void>("quit");

/** Shows the window after the first paint, so the start never flashes white. */
export async function showWindow(): Promise<void> {
  if (!isTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().show();
  await getCurrentWindow().setFocus();
}

/** null when the platform has no autostart (browser preview). */
export async function autostartEnabled(): Promise<boolean | null> {
  if (!isTauri) return null;
  const { isEnabled } = await import("@tauri-apps/plugin-autostart");
  return isEnabled();
}

export async function setAutostart(on: boolean): Promise<void> {
  if (!isTauri) return;
  const { enable, disable } = await import("@tauri-apps/plugin-autostart");
  await (on ? enable() : disable());
}

/** Subscribes to a backend event; a no-op in the browser preview. */
export async function on(event: "game-exited" | "library-changed", cb: () => void): Promise<() => void> {
  if (!isTauri) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen(event, cb);
}

// ── Desktop mode ─────────────────────────────────────────────────

export type ArtKind = "cover" | "background" | "logo";

/** Fields a person can change in desktop mode; only the ones present change. */
export interface Edit {
  name?: string;
  description?: string;
  genres?: string[];
  catalogs?: string[];
  hidden?: boolean;
}

/** A game exe found by a folder scan. */
export interface Candidate {
  exe: string;
  name: string;
  folder: string;
}

export const setView = (desktop: boolean) => invoke<void>("set_view", { desktop });
export const saveEdit = (id: string, patch: Edit) => invoke<void>("save_edit", { id, patch });
export const setArt = (id: string, kind: ArtKind, source: string) => invoke<void>("set_art", { id, kind, source });
export const removeGame = (id: string) => invoke<void>("remove_game", { id });
export const addGames = async (exes: string[]) => (await invoke<string[]>("add_games", { exes })) ?? [];
export const scanFolder = async (folder: string) => (await invoke<Candidate[]>("scan_folder", { folder })) ?? [];

/** A native file or folder picker. Null when cancelled, or in the browser preview. */
export async function chooseFile(opts: { title: string; extensions?: string[]; directory?: boolean }): Promise<string | null> {
  if (!isTauri) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({
    title: opts.title,
    directory: opts.directory,
    multiple: false,
    filters: opts.extensions ? [{ name: "Files", extensions: opts.extensions }] : undefined,
  });
  return typeof picked === "string" ? picked : null;
}

/** One game found online. */
export interface Match {
  source: "steam" | "launchbox" | "steamgriddb";
  id: string;
  name: string;
  platform: string;
  image: string | null;
}

/** One image to choose from. `source` reads like "Steam" or "LaunchBox · Europe". */
export interface Choice {
  url: string;
  thumb: string;
  source: string;
}

/** Everything one match offers: images by kind, and its description and genres. */
export interface Choices {
  cover: Choice[];
  background: Choice[];
  logo: Choice[];
  description: string;
  genres: string[];
  /** Why SteamGridDB added nothing, when it was asked and failed. */
  note: string | null;
}

/** Where "Change art" starts: the best match for the game, and the name it searched by. */
export interface ArtStart {
  query: string;
  found: Match | null;
  steamGridDb: boolean;
}

const CDN = "https://cdn.cloudflare.steamstatic.com/steam/apps";
/** The browser preview's sample games are "sample-<Steam app id>". */
const sampleApp = (game: string) => game.replace(/^sample-/, "");

export const findOnline = async (query: string) => (await invoke<Match[]>("find_online", { query })) ?? [];

export async function artMatch(game: string): Promise<ArtStart> {
  const preview: ArtStart = {
    query: "",
    found: { source: "steam", id: sampleApp(game), name: "", platform: "Steam", image: null },
    steamGridDb: false,
  };
  return (await invoke<ArtStart>("art_match", { game })) ?? preview;
}

export async function artChoices(source: string, id: string): Promise<Choices> {
  const one = (url: string): Choice[] => [{ url, thumb: url, source: "Steam" }];
  const preview: Choices = {
    cover: one(`${CDN}/${id}/library_600x900_2x.jpg`),
    background: one(`${CDN}/${id}/library_hero.jpg`),
    logo: one(`${CDN}/${id}/logo.png`),
    description: "",
    genres: [],
    note: null,
  };
  return (await invoke<Choices>("art_choices", { source, id })) ?? preview;
}

export const resetArt = (id: string, kind: ArtKind) => invoke<void>("reset_art", { id, kind });
export const setSteamGridDbKey = (key: string) => invoke<void>("set_steam_grid_db_key", { key });
