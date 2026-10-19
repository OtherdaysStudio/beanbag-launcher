import { useEffect, useState } from "react";
import { time } from "../lib/format";

export function Clock() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    let t: ReturnType<typeof setTimeout>;
    const tick = () => {
      setNow(new Date());
      t = setTimeout(tick, 60_000 - (Date.now() % 60_000));
    };
    t = setTimeout(tick, 60_000 - (Date.now() % 60_000));
    return () => clearTimeout(t);
  }, []);
  return <time className="clock">{time(now)}</time>;
}

export type Hint = [glyph: string, label: string];

/** The Menu button's three lines, drawn: the "≡" character sits low in most fonts. */
const MENU_ICON = (
  <svg viewBox="0 0 10 8" width="0.75em" height="0.6em" aria-hidden="true">
    <path d="M1 1h8M1 4h8M1 7h8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

/** Button glyphs for the controller, key caps for the keyboard, bottom right. */
export function Hints({ hints, keys }: { hints: Hint[]; keys: boolean }) {
  return (
    <footer className="hints" data-keys={keys || undefined}>
      {hints.map(([glyph, label]) => (
        <span key={glyph} className="hint">
          <span className="glyph" data-glyph={glyph}>
            {glyph === "≡" ? MENU_ICON : glyph}
          </span>
          {label}
        </span>
      ))}
    </footer>
  );
}
