/** Share of a logo's visible pixels that are light (lightness 0.5 or more), weighted by alpha.
 *  1 for an empty image, so it is left alone. */
export function lightShare(rgba: ArrayLike<number>): number {
  let light = 0;
  let weight = 0;
  for (let i = 0; i < rgba.length; i += 4) {
    const a = rgba[i + 3] / 255;
    if (a < 0.5) continue;
    // Rec. 709 weights in whole numbers, so pure white comes out at exactly 1.
    const l = (2126 * rgba[i] + 7152 * rgba[i + 1] + 722 * rgba[i + 2]) / 2_550_000;
    weight += a;
    if (l >= 0.5) light += a;
  }
  return weight ? light / weight : 1;
}

/** A logo with almost no light parts disappears on dark art. Measured on 18 real logos: The
 *  Witcher 3 and Valhalla sit under 10%; every logo that reads well, gold with dark outlines
 *  or white with black ones, has 25% or more. */
export const isDark = (share: number) => share < 0.1;
