import { describe, expect, it } from "vitest";
import { colourOf, COLOURS, paletteOf } from "./palette";

/** RGBA pixels: `count` of each colour, in order. */
function pixels(...runs: [number, [number, number, number, number?]][]): number[] {
  const out: number[] = [];
  for (const [count, [r, g, b, a = 255]] of runs) for (let i = 0; i < count; i++) out.push(r, g, b, a);
  return out;
}

describe("colourOf", () => {
  it("names the colours people would", () => {
    expect(colourOf(229, 72, 77)).toBe("red");
    expect(colourOf(247, 107, 21)).toBe("orange");
    expect(colourOf(245, 197, 24)).toBe("yellow");
    expect(colourOf(48, 164, 108)).toBe("green");
    expect(colourOf(18, 165, 148)).toBe("teal");
    expect(colourOf(58, 134, 255)).toBe("blue");
    expect(colourOf(142, 78, 198)).toBe("purple");
    expect(colourOf(233, 61, 130)).toBe("pink");
  });

  it("calls dark and muted oranges brown, and greys by how light they are", () => {
    expect(colourOf(110, 70, 35)).toBe("brown");
    expect(colourOf(210, 180, 140)).toBe("brown");
    expect(colourOf(100, 90, 20)).toBe("brown");
    expect(colourOf(12, 12, 14)).toBe("black");
    expect(colourOf(128, 128, 132)).toBe("grey");
    expect(colourOf(245, 244, 240)).toBe("white");
  });

  it("calls a pale red pink, and keeps pale oranges orange", () => {
    expect(colourOf(250, 200, 205)).toBe("pink");
    expect(colourOf(250, 218, 196)).toBe("orange");
  });

  it("has a swatch for every colour it can name", () => {
    const named = new Set(COLOURS.map((c) => c.id));
    for (const [r, g, b] of [
      [0, 0, 0],
      [255, 255, 255],
      [255, 0, 0],
      [0, 255, 0],
      [0, 0, 255],
      [120, 60, 20],
      [128, 128, 128],
    ]) {
      expect(named).toContain(colourOf(r, g, b));
    }
  });
});

describe("paletteOf", () => {
  it("lists the colours that cover a fair part of the picture, biggest first", () => {
    const p = paletteOf(pixels([60, [58, 134, 255]], [30, [245, 197, 24]], [10, [229, 72, 77]]));
    expect(p.colours).toEqual(["blue", "yellow"]);
  });

  it("keeps at most three and always the biggest", () => {
    const p = paletteOf(pixels([25, [58, 134, 255]], [25, [245, 197, 24]], [25, [229, 72, 77]], [25, [48, 164, 108]]));
    expect(p.colours).toHaveLength(3);
    expect(paletteOf(pixels([100, [58, 134, 255]])).colours).toEqual(["blue"]);
  });

  it("ignores the see-through pixels around the folder", () => {
    const p = paletteOf(pixels([90, [0, 0, 0, 0]], [10, [48, 164, 108]]));
    expect(p.colours).toEqual(["green"]);
  });

  it("says whether the picture is light or dark overall", () => {
    expect(paletteOf(pixels([100, [20, 24, 40]])).tone).toBe("dark");
    expect(paletteOf(pixels([100, [245, 197, 24]])).tone).toBe("light");
    expect(paletteOf([])).toEqual({ colours: [], tone: "light" });
  });
});
