import type { ArtKind, Choices, Match } from "./platform";

export const ART_KINDS: readonly ArtKind[] = ["cover", "background", "logo"];
export const ART_LABEL: Record<ArtKind, string> = { cover: "Cover", background: "Background", logo: "Logo" };
export const SOURCE_LABEL: Record<Match["source"], string> = { steam: "Steam", launchbox: "LaunchBox", steamgriddb: "SteamGridDB" };
/** The first tile of every grid: the game's own art, without a chosen image. */
export const ORIGINAL = "original";

const SOURCE_ORDER = ["Steam", "LaunchBox", "SteamGridDB"];

/** "Images from Steam and SteamGridDB": every source the images came from, naming the
 *  matched game only when it differs from this one. */
export function matchLine(name: string, m: Match, choices: Choices | null): string {
  const found = new Set(
    choices ? [...choices.cover, ...choices.background, ...choices.logo].map((c) => c.source.split(" · ")[0]) : [SOURCE_LABEL[m.source]],
  );
  const sources = SOURCE_ORDER.filter((s) => found.has(s));
  const from = sources.length > 1 ? `${sources.slice(0, -1).join(", ")} and ${sources.at(-1)}` : (sources[0] ?? SOURCE_LABEL[m.source]);
  const same = !m.name || m.name.toLowerCase() === name.toLowerCase();
  return same ? `Images from ${from}` : `Images for ${m.name} from ${from}`;
}
