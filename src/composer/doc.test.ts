import { describe, expect, it } from "vitest";
import {
  addLayer,
  backgroundColor,
  bringForward,
  bringToFront,
  coveringTop,
  duplicateLayer,
  emptyDoc,
  FALLBACK_PARTS,
  imageBox,
  indexOf,
  layerLabel,
  linear,
  makeEmoji,
  makeFill,
  makeImage,
  makePattern,
  makeShape,
  makeText,
  MAX_LAYERS,
  moveLayer,
  parseDoc,
  patchLayer,
  removeLayer,
  sendBackward,
  sendToBack,
  solid,
  suggestName,
  type Doc,
} from "./doc";

const three = (): Doc => {
  let d = emptyDoc();
  d = addLayer(d, makeFill(solid("#ff0000")));
  d = addLayer(d, makeText("Hello", 512, 560, "#ffffff"));
  d = addLayer(d, makeEmoji("🐶", 400, 400));
  return d;
};

describe("the design document", () => {
  it("adds, changes and removes layers without touching the old document", () => {
    const d = three();
    expect(d.layers.map((l) => l.kind)).toEqual(["fill", "text", "emoji"]);
    const id = d.layers[1].id;
    const changed = patchLayer(d, id, { x: 10 });
    expect(changed.layers[1]).toMatchObject({ x: 10 });
    expect(d.layers[1]).toMatchObject({ x: 512 });
    expect(patchLayer(d, "nope", { x: 1 })).toBe(d);
    expect(removeLayer(d, id).layers.map((l) => l.kind)).toEqual(["fill", "emoji"]);
    expect(removeLayer(d, "nope")).toBe(d);
  });

  it("restacks layers", () => {
    const d = three();
    const [fill, text, emoji] = d.layers.map((l) => l.id);
    expect(bringForward(d, fill).layers.map((l) => l.id)).toEqual([text, fill, emoji]);
    expect(sendBackward(d, emoji).layers.map((l) => l.id)).toEqual([fill, emoji, text]);
    expect(bringToFront(d, fill).layers.map((l) => l.id)).toEqual([text, emoji, fill]);
    expect(sendToBack(d, emoji).layers.map((l) => l.id)).toEqual([emoji, fill, text]);
    expect(moveLayer(d, text, 99).layers.map((l) => l.id)).toEqual([fill, emoji, text]);
    expect(moveLayer(d, fill, 0)).toBe(d);
  });

  it("duplicates a layer just above itself, nudged, with a new id", () => {
    const d = three();
    const text = d.layers[1];
    const { doc, id } = duplicateLayer(d, text.id);
    expect(doc.layers).toHaveLength(4);
    expect(indexOf(doc, id!)).toBe(2);
    expect(doc.layers[2]).toMatchObject({ kind: "text", text: "Hello", x: 540, y: 588 });
    expect(id).not.toBe(text.id);
  });

  it("stops adding at the limit", () => {
    let d = emptyDoc();
    for (let i = 0; i < MAX_LAYERS + 5; i++) d = addLayer(d, makeEmoji("⭐", i, i));
    expect(d.layers).toHaveLength(MAX_LAYERS);
    expect(duplicateLayer(d, d.layers[0].id).id).toBeNull();
  });

  it("puts new patterns above the backgrounds and below everything else", () => {
    let d = three();
    expect(coveringTop(d)).toBe(1);
    d = addLayer(d, makePattern("stripes"), coveringTop(d));
    expect(d.layers.map((l) => l.kind)).toEqual(["fill", "pattern", "text", "emoji"]);
    expect(coveringTop(d)).toBe(2);
  });

  it("names layers the way the list shows them", () => {
    const d = three();
    expect(layerLabel(d.layers[0], 0)).toBe("Background");
    expect(layerLabel(makeFill(linear(90, "#000", "#fff")), 3)).toBe("Gradient");
    expect(layerLabel(d.layers[1])).toBe("Hello");
    expect(layerLabel(makeText("A very long label that keeps on going", 0, 0, "#fff"))).toBe("A very long label tha…");
    expect(layerLabel(makeShape("star", 0, 0, "#fff"))).toBe("Star");
    expect(layerLabel({ ...makeShape("star", 0, 0, "#fff"), name: "Logo" })).toBe("Logo");
  });

  it("suggests a name from the words on it", () => {
    expect(suggestName(three())).toBe("Hello");
    expect(suggestName(emptyDoc())).toBeNull();
    expect(backgroundColor(three())).toBe("#ff0000");
    expect(backgroundColor(emptyDoc())).toBeNull();
  });

  it("places a picture over the whole folder, or a cut-out on the front", () => {
    const cover = imageBox(1600, 1000, FALLBACK_PARTS, true);
    expect(cover.w).toBeGreaterThanOrEqual(FALLBACK_PARTS.folder[2] - FALLBACK_PARTS.folder[0]);
    expect(cover.h).toBeGreaterThanOrEqual(FALLBACK_PARTS.folder[3] - FALLBACK_PARTS.folder[1] - 0.001);
    expect(cover.w / cover.h).toBeCloseTo(1.6, 5);
    const logo = imageBox(500, 500, FALLBACK_PARTS, false);
    expect(logo.x).toBe(512);
    expect(logo.y).toBe(567);
    expect(logo.w).toBeLessThan(700);
  });
});

describe("reading a design back", () => {
  it("round-trips every kind of layer", () => {
    let d = three();
    d = addLayer(d, makeShape("ring", 300, 300, "#00ff00"));
    d = addLayer(d, makePattern("confetti"));
    d = addLayer(d, makeImage("data:image/png;base64,AAAA", 20, 10, { x: 1, y: 2, w: 3, h: 4 }));
    const back = parseDoc(JSON.parse(JSON.stringify(d)));
    expect(back).toEqual(d);
  });

  it("refuses what isn't a design, and a design from a newer version", () => {
    expect(parseDoc(null)).toBeNull();
    expect(parseDoc({ layers: "x" })).toBeNull();
    expect(parseDoc({ version: 99, layers: [] })).toBeNull();
    expect(parseDoc({ layers: [] })).toEqual({ version: 1, shape: "folder", layers: [] });
  });

  it("drops what it can't draw and keeps numbers sensible", () => {
    const d = parseDoc({
      shape: "free",
      layers: [
        { kind: "sparkle" },
        { kind: "emoji", char: "" },
        { kind: "image", src: "https://example.com/x.png" },
        { kind: "fill", paint: { type: "linear", angle: 90, stops: [{ at: 0, color: "#fff" }] }, opacity: 7 },
        { kind: "text", text: "Hi", size: -5, weight: 460, blend: "weird", paint: { type: "solid", color: "nope" } },
        { kind: "shape", id: "dup", shape: "blob" },
        { kind: "shape", id: "dup", shape: "star", points: 1000 },
      ],
    })!;
    expect(d.shape).toBe("free");
    expect(d.layers.map((l) => l.kind)).toEqual(["fill", "text", "shape", "shape"]);
    const [fill, text, a, b] = d.layers;
    expect(fill).toMatchObject({ opacity: 1, paint: { type: "solid", color: "#3a86ff" } });
    expect(text).toMatchObject({ size: 4, weight: 500, blend: "normal", paint: { type: "solid", color: "#000000" } });
    expect(a).toMatchObject({ shape: "rect" });
    expect(b).toMatchObject({ shape: "star", points: 40 });
    expect(a.id).not.toBe(b.id);
  });
});
