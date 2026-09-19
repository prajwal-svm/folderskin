import { describe, expect, it } from "vitest";
import { NO_FX, type ImageFx } from "./doc";
import { applyMatrix, blurPixels, blurRadius, compose, fxMatrix, hasFx } from "./imagefx";

const px = (...rgba: number[]) => new Uint8ClampedArray(rgba);
const fx = (patch: Partial<ImageFx>): ImageFx => ({ ...NO_FX, ...patch });

describe("picture adjustments", () => {
  it("leaves a picture alone with nothing set", () => {
    expect(hasFx(NO_FX)).toBe(false);
    const d = px(10, 200, 30, 255);
    applyMatrix(d, fxMatrix(NO_FX));
    expect([...d]).toEqual([10, 200, 30, 255]);
  });

  it("brightens, darkens and inverts", () => {
    const d = px(100, 100, 100, 255);
    applyMatrix(d, fxMatrix(fx({ brightness: 50 })));
    expect([...d]).toEqual([150, 150, 150, 255]);
    const e = px(0, 128, 255, 200);
    applyMatrix(e, fxMatrix(fx({ invert: 100 })));
    expect([...e]).toEqual([255, 127, 0, 200]);
  });

  it("takes the colour out and puts contrast in", () => {
    const g = px(255, 0, 0, 255);
    applyMatrix(g, fxMatrix(fx({ grayscale: 100 })));
    expect(g[0]).toBe(g[1]);
    expect(g[1]).toBe(g[2]);
    const flat = px(40, 90, 200, 255);
    applyMatrix(flat, fxMatrix(fx({ contrast: -100 })));
    expect([...flat]).toEqual([128, 128, 128, 255]);
  });

  it("composes matrices in order", () => {
    // Brighten then invert is not invert then brighten.
    const a = fxMatrix(fx({ brightness: 50, invert: 100 }));
    const d = px(100, 100, 100, 255);
    applyMatrix(d, a);
    expect(d[0]).toBe(105);
    const id = compose(fxMatrix(NO_FX), fxMatrix(NO_FX));
    expect(id).toEqual(fxMatrix(NO_FX));
  });

  it("blurs without darkening transparent edges", () => {
    // A white dot on transparency: it spreads, and every pixel it reaches stays white.
    const w = 9;
    const d = new Uint8ClampedArray(w * w * 4);
    const mid = (4 * w + 4) * 4;
    d.set([255, 255, 255, 255], mid);
    blurPixels(d, w, w, 4);
    expect(d[mid + 3]).toBeLessThan(255);
    expect(d[mid + 3]).toBeGreaterThan(0);
    const beside = mid + 4;
    expect(d[beside + 3]).toBeGreaterThan(0);
    expect(d[beside]).toBe(255);
    // A radius under a pixel does nothing.
    const e = px(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16);
    blurPixels(e, 2, 2, 0.5);
    expect([...e]).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
  });

  it("scales blur with the picture", () => {
    expect(blurRadius(fx({ blur: 10 }), 2048, 1000)).toBe(20);
  });
});
