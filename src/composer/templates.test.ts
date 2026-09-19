import { describe, expect, it } from "vitest";
import { frame, unframe } from "./body";
import { FALLBACK_PARTS, parseDoc } from "./doc";
import { EMOJI, searchEmoji } from "./emoji";
import { confettiColors, grainPixels, seeded } from "./patterns";
import { TEMPLATES } from "./templates";

describe("templates", () => {
  it("each makes a design that reads back the same", () => {
    const ids = new Set<string>();
    for (const t of TEMPLATES) {
      expect(ids.has(t.id), t.id).toBe(false);
      ids.add(t.id);
      const doc = t.make(FALLBACK_PARTS, { src: "data:image/png;base64,AAAA", width: 800, height: 600, alpha: false });
      expect(doc.layers.length, t.id).toBeGreaterThan(0);
      expect(parseDoc(JSON.parse(JSON.stringify(doc))), t.id).toEqual(doc);
    }
  });

  it("start new layers on the folder", () => {
    for (const t of TEMPLATES) {
      for (const l of t.make(FALLBACK_PARTS).layers) {
        if (!("x" in l)) continue;
        expect(l.x, `${t.id} ${l.kind}`).toBeGreaterThan(0);
        expect(l.x, `${t.id} ${l.kind}`).toBeLessThan(1024);
        expect(l.y, `${t.id} ${l.kind}`).toBeGreaterThan(0);
        expect(l.y, `${t.id} ${l.kind}`).toBeLessThan(1024);
      }
    }
  });
});

describe("the raw body a design is sent in", () => {
  it("puts the header first and the picture after it", () => {
    const png = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 1, 2, 3]);
    const body = frame({ name: "Ünïcode ✈️", sizes: [16] }, png);
    const head = new TextEncoder().encode(JSON.stringify({ name: "Ünïcode ✈️", sizes: [16] }));
    expect(new DataView(body.buffer).getUint32(0, true)).toBe(head.length);
    const back = unframe(body);
    expect(back.header).toEqual({ name: "Ünïcode ✈️", sizes: [16] });
    expect([...back.png]).toEqual([...png]);
    expect(() => unframe(new Uint8Array([9, 0, 0, 0, 1]))).toThrow();
  });
});

describe("patterns and emoji", () => {
  it("draws the same randomness from the same seed", () => {
    const a = seeded(42);
    const b = seeded(42);
    const xs = [a(), a(), a()];
    expect([b(), b(), b()]).toEqual(xs);
    expect(xs.every((x) => x >= 0 && x < 1)).toBe(true);
    expect([...grainPixels(4, 7, "#ffffff")]).toEqual([...grainPixels(4, 7, "#ffffff")]);
  });

  it("starts confetti from the layer's colour", () => {
    const c = confettiColors("#ff0000");
    expect(c[0]).toBe("#ff0000");
    expect(new Set(c).size).toBe(5);
  });

  it("finds emoji by what they are", () => {
    expect(searchEmoji("dog")).toContain("🐶");
    expect(searchEmoji("tax")).toContain("🧾");
    expect(searchEmoji("")).toEqual([]);
    const all = EMOJI.flatMap((g) => g.items.map(([c]) => c));
    expect(new Set(all).size).toBe(all.length);
  });
});
