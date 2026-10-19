import { useEffect, useState } from "react";

interface Layer {
  id: number;
  src: string;
  ready: boolean;
}

let seq = 0;

/**
 * Full-bleed art that crossfades when `src` changes. The change waits a beat,
 * so holding the stick across a row does not decode every image on the way.
 */
export function Backdrop({ src, className = "" }: { src?: string; className?: string }) {
  const [layers, setLayers] = useState<Layer[]>([]);

  useEffect(() => {
    if (!src) {
      setLayers([]);
      return;
    }
    const t = setTimeout(() => {
      setLayers((ls) => {
        if (ls.at(-1)?.src === src) return ls;
        return [...ls.filter((l) => l.ready).slice(-1), { id: ++seq, src, ready: false }];
      });
    }, 110);
    return () => clearTimeout(t);
  }, [src]);

  const settle = (id: number, ok: boolean) =>
    setLayers((ls) => (ok ? ls.map((l) => (l.id === id ? { ...l, ready: true } : l)) : ls.filter((l) => l.id !== id)));

  return (
    <div className={`backdrop ${className}`} aria-hidden>
      {layers.map((l) => (
        <img
          key={l.id}
          src={l.src}
          alt=""
          decoding="async"
          draggable={false}
          data-ready={l.ready || undefined}
          onLoad={() => settle(l.id, true)}
          onError={() => settle(l.id, false)}
        />
      ))}
    </div>
  );
}
