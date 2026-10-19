import { useState } from "react";
import type { Candidate } from "../lib/platform";

interface ScanDialogProps {
  folder: string;
  candidates: Candidate[];
  onAdd: (exes: string[]) => void;
  onCancel: () => void;
}

/** What a folder scan found. Everything starts ticked; untick what is not a game. */
export function ScanDialog({ folder, candidates, onAdd, onCancel }: ScanDialogProps) {
  const [picked, setPicked] = useState(() => new Set(candidates.map((c) => c.exe)));
  const toggle = (exe: string) =>
    setPicked((p) => {
      const next = new Set(p);
      if (next.has(exe)) next.delete(exe);
      else next.add(exe);
      return next;
    });
  return (
    <div className="dt-dialog" role="dialog" aria-modal="true" aria-labelledby="scan-title" onKeyDown={(e) => e.key === "Escape" && onCancel()}>
      <div className="dt-dialog-panel">
        <header className="dt-dialog-head">
          <h2 id="scan-title">{candidates.length ? `${candidates.length} games found` : "No new games found"}</h2>
          <p>{folder}</p>
        </header>
        <div className="dt-dialog-list">
          {candidates.map((c) => (
            <label key={c.exe} className="dt-candidate">
              <input type="checkbox" checked={picked.has(c.exe)} onChange={() => toggle(c.exe)} />
              <span>
                {c.name}
                <small>{c.exe}</small>
              </span>
            </label>
          ))}
        </div>
        <footer className="dt-dialog-foot">
          <button type="button" className="dt-button" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="dt-button dt-button-primary"
            disabled={!picked.size}
            onClick={() => onAdd([...picked])}
          >
            {picked.size === 1 ? "Add 1 game" : `Add ${picked.size} games`}
          </button>
        </footer>
      </div>
    </div>
  );
}
