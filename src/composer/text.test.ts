import { describe, expect, it } from "vitest";
import { makeText, type TextLayer } from "./doc";
import { fontOf, graphemes, layoutText, linesOf, type Measure } from "./text";

/** Every letter half the font size wide: easy numbers to check. */
const mono: Measure = (font, text) => {
  const size = Number(/(\d+(?:\.\d+)?)px/.exec(font)![1]);
  return graphemes(text).length * size * 0.5;
};

const text = (t: string, patch: Partial<TextLayer> = {}): TextLayer => ({ ...makeText(t, 512, 512, "#fff"), size: 100, ...patch });

describe("text layout", () => {
  it("writes the font the canvas draws with", () => {
    expect(fontOf({ italic: true, weight: 800, size: 64, font: "mono" })).toMatch(/^italic 800 64px ui-monospace/);
    expect(fontOf({ italic: false, weight: 400, size: 10, font: "custom:My Font" })).toBe('400 10px "My Font", system-ui, sans-serif');
  });

  it("lays straight lines out centred, left or right", () => {
    const l = layoutText(text("abcd\nab", { lineHeight: 1.2 }), mono);
    expect(l.glyphs).toBeNull();
    expect(l.w).toBe(200);
    expect(l.h).toBe(240);
    expect(l.lines).toEqual([
      { text: "abcd", x: -100, y: -60, width: 200 },
      { text: "ab", x: -50, y: 60, width: 100 },
    ]);
    const left = layoutText(text("abcd\nab", { align: "left" }), mono);
    expect(left.lines[1].x).toBe(-100);
    const right = layoutText(text("abcd\nab", { align: "right" }), mono);
    expect(right.lines[1].x).toBe(0);
  });

  it("sets capitals when asked", () => {
    expect(linesOf({ text: "hi\nthere", upper: true })).toEqual(["HI", "THERE"]);
  });

  it("spaces letters out one at a time", () => {
    const l = layoutText(text("abc", { spacing: 0.2 }), mono);
    expect(l.glyphs).toHaveLength(3);
    // 3 letters of 50 with 2 gaps of 20.
    expect(l.w).toBeCloseTo(190, 6);
    const xs = l.glyphs!.map((g) => g.x);
    expect(xs[1] - xs[0]).toBeCloseTo(70, 6);
    expect(xs[0] + xs[2]).toBeCloseTo(0, 6);
  });

  it("bends text into an arch or a smile, centred", () => {
    const arch = layoutText(text("abcdef", { curve: 60 }), mono);
    const g = arch.glyphs!;
    expect(g[0].rot).toBeLessThan(0);
    expect(g[5].rot).toBeGreaterThan(0);
    // An arch: the middle letters sit higher than the ends.
    expect(g[2].y).toBeLessThan(g[0].y);
    const smile = layoutText(text("abcdef", { curve: -60 }), mono);
    expect(smile.glyphs![2].y).toBeGreaterThan(smile.glyphs![0].y);
    // Bent text takes more height than the same text straight.
    expect(arch.h).toBeGreaterThan(layoutText(text("abcdef"), mono).h);
  });

  it("keeps an emoji made of several code points in one piece", () => {
    expect(graphemes("a👩‍💻b")).toHaveLength(3);
  });

  it("gives empty text a box that can still be picked", () => {
    const l = layoutText(text(""), mono);
    expect(l.w).toBeGreaterThan(0);
    expect(l.h).toBeGreaterThan(0);
  });
});
