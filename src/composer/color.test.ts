import { describe, expect, it } from "vitest";
import { alphaOf, cssColor, hsvToRgb, inkOn, luminance, mix, normalizeColor, parseColor, rgbToHsv, toHex, withAlpha } from "./color";

describe("colours", () => {
  it("reads every way of writing a colour", () => {
    expect(parseColor("#f80")).toEqual({ r: 255, g: 136, b: 0, a: 1 });
    expect(parseColor("#ff880080")?.a).toBeCloseTo(0.502, 2);
    expect(parseColor("#FF8800")).toEqual({ r: 255, g: 136, b: 0, a: 1 });
    expect(parseColor("rgba(10, 20, 30, 0.5)")).toEqual({ r: 10, g: 20, b: 30, a: 0.5 });
    expect(parseColor("rgb(100% 0% 0% / 25%)")).toEqual({ r: 255, g: 0, b: 0, a: 0.25 });
    expect(parseColor("transparent")?.a).toBe(0);
    expect(parseColor("blue")).toBeNull();
    expect(parseColor("#12345")).toBeNull();
  });

  it("writes opaque colours short and see-through ones with their alpha", () => {
    expect(toHex({ r: 58, g: 134, b: 255, a: 1 })).toBe("#3a86ff");
    expect(toHex({ r: 255, g: 255, b: 255, a: 0.5 })).toBe("#ffffff80");
    expect(normalizeColor("#ABC", "#000000")).toBe("#aabbcc");
    expect(normalizeColor(42, "#000000")).toBe("#000000");
    expect(withAlpha("#3a86ff", 0)).toBe("#3a86ff00");
    expect(alphaOf("#3a86ff80")).toBeCloseTo(0.5, 2);
  });

  it("gives every canvas a colour it can read", () => {
    expect(cssColor("#3a86ff80")).toBe("rgba(58,134,255,0.502)");
    expect(cssColor("nonsense")).toBe("rgba(0,0,0,0)");
  });

  it("round-trips through HSV", () => {
    for (const hex of ["#3a86ff", "#ff5a5f", "#34c77b", "#000000", "#ffffff", "#808080"]) {
      const c = parseColor(hex)!;
      expect(toHex(hsvToRgb(rgbToHsv(c)))).toBe(hex);
    }
    expect(rgbToHsv({ r: 255, g: 0, b: 0, a: 1 })).toEqual({ h: 0, s: 1, v: 1 });
  });

  it("picks the ink that reads on a background", () => {
    expect(luminance("#ffffff")).toBeCloseTo(1, 5);
    expect(luminance("#000000")).toBe(0);
    expect(inkOn("#ffffff")).not.toBe("#ffffff");
    expect(inkOn("#1d3557")).toBe("#ffffff");
    expect(inkOn("#ffffff10")).toBe("#ffffff");
  });

  it("mixes two colours", () => {
    expect(mix("#000000", "#ffffff", 0.5)).toBe("#808080");
    expect(mix("#ff0000", "#0000ff", 0)).toBe("#ff0000");
  });
});
