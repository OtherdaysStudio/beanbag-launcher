import { useCallback, useEffect, useState } from "react";
import { ORIGINAL } from "../lib/art";
import { artChoices, artMatch, resetArt, saveEdit, setArt, type ArtKind, type ArtStart, type Choices, type Match } from "../lib/platform";

/**
 * "Change art" for one game, shared by big screen and desktop mode: finds the game online,
 * lists every image its sources have, and applies a pick at once.
 */
export function useArtPicker(gameId: string, onChanged: () => void) {
  const [start, setStart] = useState<ArtStart | null>(null);
  const [match, setMatch] = useState<Match | null>(null);
  const [choices, setChoices] = useState<Choices | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  /** What this visit applied, by kind: an image URL, or ORIGINAL. */
  const [applied, setApplied] = useState<Partial<Record<ArtKind, string>>>({});
  /** The image being saved right now. */
  const [saving, setSaving] = useState<string | null>(null);
  const [detailsUsed, setDetailsUsed] = useState(false);

  const show = useCallback((m: Match | null) => {
    setMatch(m);
    setChoices(null);
    setError(null);
    setDetailsUsed(false);
    if (!m) {
      setLoading(false);
      return;
    }
    setLoading(true);
    artChoices(m.source, m.id)
      .then(setChoices)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    let live = true;
    artMatch(gameId)
      .then((s) => {
        if (!live) return;
        setStart(s);
        show(s.found);
      })
      .catch((e) => {
        if (!live) return;
        setError(String(e));
        setLoading(false);
      });
    return () => {
      live = false;
    };
  }, [gameId, show]);

  const save = async (kind: ArtKind, value: string, work: () => Promise<void>) => {
    setSaving(value);
    setError(null);
    try {
      await work();
      setApplied((a) => ({ ...a, [kind]: value }));
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(null);
    }
  };

  const pick = (kind: ArtKind, url: string) =>
    url === ORIGINAL ? save(kind, ORIGINAL, () => resetArt(gameId, kind)) : save(kind, url, () => setArt(gameId, kind, url));

  const useDetails = async () => {
    if (!choices) return;
    setError(null);
    try {
      await saveEdit(gameId, {
        description: choices.description || undefined,
        genres: choices.genres.length ? choices.genres : undefined,
      });
      setDetailsUsed(true);
      onChanged();
    } catch (e) {
      setError(String(e));
    }
  };

  return { start, match, choices, loading, error, applied, saving, detailsUsed, show, pick, useDetails };
}

export type ArtPicker = ReturnType<typeof useArtPicker>;
