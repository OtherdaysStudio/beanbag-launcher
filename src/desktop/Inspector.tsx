import { useEffect, useState } from "react";
import { FindDialog } from "./FindDialog";
import { lastPlayed, playtime, size } from "../lib/format";
import { chooseFile, removeGame, saveEdit, setArt, type ArtKind, type Edit } from "../lib/platform";
import type { Game } from "../lib/types";

interface InspectorProps {
  game: Game;
  allGenres: string[];
  allCatalogs: string[];
  onPlay: (game: Game) => void;
  onChanged: () => void;
}

/** Tags with a text box: Enter adds, × removes, suggestions come from the library. */
function Chips({ label, values, options, onChange }: { label: string; values: string[]; options: string[]; onChange: (v: string[]) => void }) {
  const [draft, setDraft] = useState("");
  const id = `chips-${label}`;
  const add = () => {
    const v = draft.trim();
    if (v && !values.some((x) => x.toLowerCase() === v.toLowerCase())) onChange([...values, v]);
    setDraft("");
  };
  return (
    <div className="dt-field">
      <label className="dt-label" htmlFor={id}>
        {label}
      </label>
      <div className="dt-chips">
        {values.map((v) => (
          <span key={v} className="dt-chip">
            {v}
            <button type="button" aria-label={`Remove ${v}`} onClick={() => onChange(values.filter((x) => x !== v))}>
              ×
            </button>
          </span>
        ))}
        <input
          id={id}
          className="dt-chip-input"
          list={`${id}-list`}
          value={draft}
          placeholder={values.length ? "" : `Add ${label.toLowerCase()}`}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === ",") {
              e.preventDefault();
              add();
            } else if (e.key === "Backspace" && !draft && values.length) {
              onChange(values.slice(0, -1));
            }
          }}
          onBlur={add}
        />
        <datalist id={`${id}-list`}>
          {options.filter((o) => !values.includes(o)).map((o) => (
            <option key={o} value={o} />
          ))}
        </datalist>
      </div>
    </div>
  );
}

const IMAGES = [".png", ".jpg", ".jpeg", ".webp"].map((e) => e.slice(1));

/** The selected game: art, details and how it starts. Every change saves on its own. */
export function Inspector({ game, allGenres, allCatalogs, onPlay, onChanged }: InspectorProps) {
  const [name, setName] = useState(game.name);
  const [description, setDescription] = useState(game.description);
  const [genres, setGenres] = useState(game.genres);
  const [catalogs, setCatalogs] = useState(game.catalogs);
  const [state, setState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [finding, setFinding] = useState(false);

  // New values from the library (after "Find online") replace a field unless it is being typed in.
  const typing = (id: string) => document.activeElement?.id === id;
  useEffect(() => {
    if (!typing("dt-name")) setName(game.name);
  }, [game.name]);
  useEffect(() => {
    if (!typing("dt-description")) setDescription(game.description);
  }, [game.description]);
  useEffect(() => setGenres(game.genres), [game.genres]);
  useEffect(() => setCatalogs(game.catalogs), [game.catalogs]);

  const save = (patch: Edit) => {
    setState("saving");
    saveEdit(game.id, patch)
      .then(() => {
        setState("saved");
        onChanged();
      })
      .catch(() => setState("error"));
  };

  const changeArt = async (kind: ArtKind) => {
    const path = await chooseFile({ title: "Choose an image", extensions: IMAGES });
    if (!path) return;
    setState("saving");
    setArt(game.id, kind, path)
      .then(() => {
        setState("saved");
        onChanged();
      })
      .catch(() => setState("error"));
  };

  const meta = [
    game.lastPlayed ? `Played ${lastPlayed(game.lastPlayed)}` : "Not played yet",
    playtime(game.playtime),
    size(game.sizeBytes),
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <aside className="dt-inspector" aria-label={`${game.name} details`}>
      <div className="dt-art">
        <button type="button" className="dt-hero" onClick={() => changeArt("background")} aria-label="Change background">
          {game.art.hero ? <img src={game.art.hero} alt="" /> : <span className="dt-missing">No background</span>}
          <span className="dt-change">Change background…</span>
        </button>
        <button type="button" className="dt-cover" onClick={() => changeArt("cover")} aria-label="Change cover">
          {game.art.cover ? <img src={game.art.cover} alt="" /> : <span className="dt-missing">No cover</span>}
          <span className="dt-change">Change…</span>
        </button>
        <button type="button" className="dt-logo" onClick={() => changeArt("logo")} aria-label="Change logo">
          {game.art.logo ? <img src={game.art.logo} alt="" /> : <span className="dt-missing">No logo</span>}
          <span className="dt-change">Change logo…</span>
        </button>
      </div>

      <div className="dt-art-actions">
        <button type="button" className="dt-button" onClick={() => setFinding(true)}>
          Find art and details online…
        </button>
      </div>

      <div className="dt-form">
        <div className="dt-field">
          <label className="dt-label" htmlFor="dt-name">
            Name
          </label>
          <input
            id="dt-name"
            className="dt-input dt-title"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onBlur={() => name.trim() && name !== game.name && save({ name: name.trim() })}
            onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
          />
          <p className="dt-meta">{meta}</p>
        </div>

        <Chips label="Genres" values={genres} options={allGenres} onChange={(v) => (setGenres(v), save({ genres: v }))} />
        <Chips label="Catalogs" values={catalogs} options={allCatalogs} onChange={(v) => (setCatalogs(v), save({ catalogs: v }))} />

        <div className="dt-field">
          <label className="dt-label" htmlFor="dt-description">
            Description
          </label>
          <textarea
            id="dt-description"
            className="dt-input dt-textarea"
            rows={5}
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            onBlur={() => description !== game.description && save({ description })}
          />
        </div>

        <div className="dt-field">
          <span className="dt-label">Installed in</span>
          <p className="dt-path" title={game.installDir}>
            {game.installDir || "—"}
          </p>
        </div>
      </div>

      <footer className="dt-footer">
        <span className="dt-status" role="status" data-state={state}>
          {state === "saving" ? "Saving…" : state === "saved" ? "Saved" : state === "error" ? "Not saved. Try again." : ""}
        </span>
        <button type="button" className="dt-button" onClick={() => save({ hidden: !game.hidden })}>
          {game.hidden ? "Show in library" : "Hide"}
        </button>
        {game.added && (
          <button type="button" className="dt-button dt-danger" onClick={() => removeGame(game.id).then(onChanged)}>
            Remove {game.name}
          </button>
        )}
        <button type="button" className="dt-button dt-button-primary" onClick={() => onPlay(game)}>
          Play
        </button>
      </footer>
      {finding && (
        <FindDialog
          game={game}
          onClose={() => setFinding(false)}
          onChanged={() => {
            setState("saved");
            onChanged();
          }}
        />
      )}
    </aside>
  );
}
