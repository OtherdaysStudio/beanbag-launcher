import { useLayoutEffect, useRef } from "react";

/**
 * Slides `page` on the GPU so row `row` sits at `anchor` (0–1 of the screen height).
 * Row 0 always rests at the top. Returns the refs to attach.
 */
export function useGlide(row: number, deps: unknown[], anchor = 0.5) {
  const pageRef = useRef<HTMLDivElement>(null);
  const rowRefs = useRef<Array<HTMLDivElement | null>>([]);

  useLayoutEffect(() => {
    const page = pageRef.current;
    const el = rowRefs.current[row];
    if (!page || !el) return;
    const vh = window.innerHeight;
    const max = Math.max(0, page.scrollHeight - vh);
    const y = row === 0 ? 0 : Math.min(max, Math.max(0, el.offsetTop + el.offsetHeight / 2 - vh * anchor));
    page.style.transform = `translate3d(0, ${-y}px, 0)`;
  }, [row, ...deps]);

  const setRow = (r: number) => (el: HTMLDivElement | null) => {
    rowRefs.current[r] = el;
  };
  return { pageRef, setRow };
}
