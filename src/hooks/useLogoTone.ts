import { useEffect, useState } from "react";
import { isDark, lightShare } from "../lib/logoTone";

export type Tone = "dark" | "light" | "pending";

// v2: the rule changed from average lightness to the share of light pixels.
const KEY = "beanbag.logo-tone.v2";
const known: Record<string, boolean> = (() => {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}");
  } catch {
    return {};
  }
})();
const measuring = new Map<string, Promise<boolean | null>>();

/** Reads the logo's pixels once. Null when it cannot: a web image the browser will not share. */
function measure(src: string): Promise<boolean | null> {
  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      try {
        const w = 160;
        const h = Math.max(1, Math.round((w * img.naturalHeight) / Math.max(1, img.naturalWidth)));
        const canvas = document.createElement("canvas");
        canvas.width = w;
        canvas.height = h;
        const g = canvas.getContext("2d", { willReadFrequently: true });
        if (!g) return resolve(null);
        g.drawImage(img, 0, 0, w, h);
        resolve(isDark(lightShare(g.getImageData(0, 0, w, h).data)));
      } catch {
        resolve(null);
      }
    };
    img.onerror = () => resolve(null);
    img.src = src;
  });
}

const toneOf = (src: string | undefined): Tone =>
  !src ? "light" : src in known ? (known[src] ? "dark" : "light") : "pending";

/** Whether a logo is dark, remembered across starts. "pending" only until the first read. */
export function useLogoTone(src: string | undefined): Tone {
  const [tone, setTone] = useState<Tone>(() => toneOf(src));
  useEffect(() => {
    setTone(toneOf(src));
    if (!src || src in known) return;
    let live = true;
    let read = measuring.get(src);
    if (!read) {
      read = measure(src);
      measuring.set(src, read);
    }
    void read.then((dark) => {
      if (dark !== null) {
        known[src] = dark;
        try {
          localStorage.setItem(KEY, JSON.stringify(known));
        } catch {
          // Not saved: the next start reads the logo again.
        }
      }
      if (live) setTone(dark ? "dark" : "light");
    });
    return () => {
      live = false;
    };
  }, [src]);
  return tone;
}
