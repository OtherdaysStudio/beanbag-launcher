import { describe, expect, it } from "vitest";
import { isDark, lightShare } from "./logoTone";

const px = (...pixels: number[][]) => pixels.flat();
const many = (n: number, pixel: number[]) => Array<number[]>(n).fill(pixel);

describe("logo tone", () => {
  it("measures the share of light pixels", () => {
    expect(lightShare(px([0, 0, 0, 255]))).toBe(0);
    expect(lightShare(px([255, 255, 255, 255]))).toBe(1);
    expect(lightShare(px([0, 0, 0, 255], [255, 255, 255, 255]))).toBe(0.5);
  });

  it("counts only the visible pixels", () => {
    expect(lightShare(px([0, 0, 0, 255], [255, 255, 255, 0], [255, 255, 255, 40]))).toBe(0);
  });

  it("leaves an empty image alone", () => {
    expect(isDark(lightShare(px([0, 0, 0, 0])))).toBe(false);
  });

  it("flips a charcoal logo with a red mark, like The Witcher 3", () => {
    expect(isDark(lightShare(px(...many(8, [40, 40, 44, 255]), [200, 20, 20, 255], [200, 20, 20, 255])))).toBe(true);
  });

  it("keeps a gold logo with dark outlines, like Baldur's Gate 3", () => {
    const logo = px(...many(4, [30, 20, 10, 255]), ...many(3, [150, 110, 50, 255]), ...many(3, [230, 190, 110, 255]));
    expect(isDark(lightShare(logo))).toBe(false);
  });

  it("keeps a white logo with black outlines, like Red Dead Redemption 2", () => {
    expect(isDark(lightShare(px(...many(5, [10, 10, 10, 255]), ...many(5, [250, 250, 250, 255]))))).toBe(false);
  });
});
