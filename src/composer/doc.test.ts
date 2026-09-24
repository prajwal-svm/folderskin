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
  refit,
  removeLayer,
  sendBackward,
  sendToBack,
  solid,
  suggestName,
  WINDOWS_PARTS,
  type Doc,
} from "./doc";
import { inkOn } from "./color";
import { templateById } from "./templates";

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
    expect(layerLabel(makeText("A very long label that keeps on going", 0, 0, "#fff"))).toBe("A very long label that");
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
    expect(parseDoc({ layers: [] })).toEqual({ version: 1, shape: "folder", style: "mac", layers: [] });
  });

  it("keeps a colour that covers only the front, and nothing else it doesn't know", () => {
    const front = { ...makeFill(solid("#60d0ff")), part: "front" as const };
    const d = addLayer(addLayer(emptyDoc(), makeFill(solid("#0a94d3"))), front);
    expect(parseDoc(JSON.parse(JSON.stringify(d)))).toEqual(d);
    const odd = parseDoc({ layers: [{ kind: "fill", paint: { type: "solid", color: "#ffffff" }, part: "back" }] })!;
    expect(odd.layers[0]).not.toHaveProperty("part");
    expect(layerLabel(front, 1)).toBe("Front");
    // What's added lands on the front, so it's the front's colour it should stand out from.
    expect(backgroundColor(d)).toBe("#60d0ff");
  });

  it("keeps which folder a design is on, a Mac's unless it says Windows'", () => {
    expect(parseDoc({ style: "windows", layers: [] })?.style).toBe("windows");
    expect(parseDoc({ style: "linux", layers: [] })?.style).toBe("mac");
    expect(parseDoc(JSON.parse(JSON.stringify(emptyDoc("folder", "windows"))))).toEqual(emptyDoc("folder", "windows"));
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

describe("moving a design between the Mac's folder and Windows'", () => {
  const onMac = () => {
    const tab = FALLBACK_PARTS.tab;
    const front = FALLBACK_PARTS.front;
    let d = emptyDoc("folder", "mac");
    d = addLayer(d, makeFill(solid("#ff0000")));
    d = addLayer(d, makeImage("data:image/png;base64,AAAA", 1600, 1000, imageBox(1600, 1000, FALLBACK_PARTS, true)));
    d = addLayer(d, { ...makeText("IDEAS", (tab[0] + tab[2]) / 2, (tab[1] + tab[3]) / 2, "#ffffff"), size: 44 });
    d = addLayer(d, { ...makeEmoji("💡", (front[0] + front[2]) / 2, (front[1] + front[3]) / 2), size: 380 });
    return d;
  };

  it("keeps what's on the tab on the tab and what's in the middle of the front there", () => {
    const d = refit(onMac(), FALLBACK_PARTS, WINDOWS_PARTS, "windows");
    expect(d.style).toBe("windows");
    const [, , text, emoji] = d.layers;
    const tab = WINDOWS_PARTS.tab;
    const front = WINDOWS_PARTS.front;
    // Across the folder in step with its width, so near the tab's middle; down it in step with the tab.
    expect(text).toMatchObject({ y: (tab[1] + tab[3]) / 2 });
    expect(Math.abs(text.kind === "text" ? text.x - (tab[0] + tab[2]) / 2 : 99)).toBeLessThan(20);
    expect(emoji).toMatchObject({ x: (front[0] + front[2]) / 2, y: (front[1] + front[3]) / 2 });
    // Windows' folder is smaller, so they are too.
    expect(text.kind === "text" && text.size).toBeLessThan(44);
    expect(emoji.kind === "emoji" && emoji.size).toBeLessThan(380);
  });

  it("leaves what covers the canvas alone, and a picture that covered the folder still covers it", () => {
    const mac = onMac();
    const d = refit(mac, FALLBACK_PARTS, WINDOWS_PARTS, "windows");
    expect(d.layers[0]).toBe(mac.layers[0]);
    const pic = d.layers[1];
    if (pic.kind !== "image") throw new Error("not the picture");
    const [x0, y0, x1, y1] = WINDOWS_PARTS.folder;
    expect(pic.x - pic.w / 2).toBeLessThanOrEqual(x0);
    expect(pic.x + pic.w / 2).toBeGreaterThanOrEqual(x1);
    expect(pic.y - pic.h / 2).toBeLessThanOrEqual(y0);
    expect(pic.y + pic.h / 2).toBeGreaterThanOrEqual(y1);
    const back = refit(d, WINDOWS_PARTS, FALLBACK_PARTS, "mac").layers[1];
    if (back.kind !== "image") throw new Error("not the picture");
    const [mx0, my0, mx1, my1] = FALLBACK_PARTS.folder;
    expect(back.x - back.w / 2).toBeLessThanOrEqual(mx0 + 0.5);
    expect(back.x + back.w / 2).toBeGreaterThanOrEqual(mx1 - 0.5);
    expect(back.y - back.h / 2).toBeLessThanOrEqual(my0 + 0.5);
    expect(back.y + back.h / 2).toBeGreaterThanOrEqual(my1 - 0.5);
  });

  it("keeps a picture that only just covered Windows' folder covering a Mac's", () => {
    let d = emptyDoc("folder", "windows");
    d = addLayer(d, makeImage("data:image/png;base64,AAAA", 1600, 1000, imageBox(1600, 1000, WINDOWS_PARTS, true)));
    const pic = refit(d, WINDOWS_PARTS, FALLBACK_PARTS, "mac").layers[0];
    if (pic.kind !== "image") throw new Error("not the picture");
    const [x0, y0, x1, y1] = FALLBACK_PARTS.folder;
    expect(pic.x - pic.w / 2).toBeLessThanOrEqual(x0);
    expect(pic.x + pic.w / 2).toBeGreaterThanOrEqual(x1);
    expect(pic.y - pic.h / 2).toBeLessThanOrEqual(y0);
    expect(pic.y + pic.h / 2).toBeGreaterThanOrEqual(y1);
    expect(pic.w / pic.h).toBeCloseTo(1.6, 2);
  });

  it("keeps a band across the folder on its part and across the whole width", () => {
    const d = templateById("two-tone")!.make(FALLBACK_PARTS);
    const there = refit(d, FALLBACK_PARTS, WINDOWS_PARTS, "windows");
    const front = there.layers.find((l) => l.name === "Front");
    if (front?.kind !== "shape") throw new Error("not the front");
    // Below the tab, so the words on it show against the back's colour.
    expect(front.y - front.h / 2).toBeGreaterThan(WINDOWS_PARTS.tab[3]);
    expect(front.y - front.h / 2).toBeLessThanOrEqual(WINDOWS_PARTS.front[1]);
    expect(front.y + front.h / 2).toBeGreaterThanOrEqual(WINDOWS_PARTS.folder[3]);
    expect(front.x - front.w / 2).toBeLessThanOrEqual(WINDOWS_PARTS.folder[0]);
    expect(front.x + front.w / 2).toBeGreaterThanOrEqual(WINDOWS_PARTS.folder[2]);
    const back = refit(there, WINDOWS_PARTS, FALLBACK_PARTS, "mac").layers.find((l) => l.name === "Front");
    const was = d.layers.find((l) => l.name === "Front");
    if (back?.kind !== "shape" || was?.kind !== "shape") throw new Error("not the front");
    for (const k of ["x", "y", "w", "h"] as const) expect(Math.abs(back[k] - was[k]), k).toBeLessThan(0.5);
  });

  it("keeps the Photo template's caption band across Windows' folder", () => {
    const d = templateById("photo")!.make(FALLBACK_PARTS);
    const band = refit(d, FALLBACK_PARTS, WINDOWS_PARTS, "windows").layers.find((l) => l.name === "Caption band");
    if (band?.kind !== "shape") throw new Error("not the band");
    expect(band.x - band.w / 2).toBeLessThanOrEqual(WINDOWS_PARTS.folder[0]);
    expect(band.x + band.w / 2).toBeGreaterThanOrEqual(WINDOWS_PARTS.folder[2]);
    expect(band.y + band.h / 2).toBeLessThan(WINDOWS_PARTS.folder[3]);
  });

  it("starts Two-tone's front on Windows' front, which has no paper sheet to hide the split behind", () => {
    const front = templateById("two-tone")!.make(WINDOWS_PARTS).layers.find((l) => l.name === "Front");
    if (front?.kind !== "shape") throw new Error("not the front");
    expect(front.y - front.h / 2).toBe(WINDOWS_PARTS.front[1]);
    const mac = templateById("two-tone")!.make(FALLBACK_PARTS).layers.find((l) => l.name === "Front");
    if (mac?.kind !== "shape") throw new Error("not the front");
    expect(mac.y - mac.h / 2).toBeCloseTo((FALLBACK_PARTS.paper[1] + FALLBACK_PARTS.front[1]) / 2, 5);
  });

  it("makes new words stand out from Two-tone's front, the rectangle that covers it, not the colour under that", () => {
    for (const parts of [FALLBACK_PARTS, WINDOWS_PARTS]) {
      const d = templateById("two-tone")!.make(parts);
      expect(backgroundColor(d, parts.front)).toBe("#023047");
      expect(inkOn(backgroundColor(d, parts.front)!)).toBe("#ffffff");
      // Moved to the other folder, it still covers the front.
      const other = parts === FALLBACK_PARTS ? WINDOWS_PARTS : FALLBACK_PARTS;
      expect(backgroundColor(refit(d, parts, other, other === WINDOWS_PARTS ? "windows" : "mac"), other.front)).toBe("#023047");
      // Drawing the design still goes by the folder's own colour.
      expect(backgroundColor(d)).toBe("#ffb703");
    }
    // A label on the front is something on it, not its colour.
    const label = { ...makeShape("rect", 512, 560, "#ffffff"), w: 560, h: 210 };
    expect(backgroundColor(addLayer(three(), label), FALLBACK_PARTS.front)).toBe("#ff0000");
  });

  it("comes back to about where it was", () => {
    const d = onMac();
    const there = refit(d, FALLBACK_PARTS, WINDOWS_PARTS, "windows");
    const back = refit(there, WINDOWS_PARTS, FALLBACK_PARTS, "mac");
    for (const i of [2, 3]) {
      const a = d.layers[i];
      const b = back.layers[i];
      if ((a.kind !== "text" && a.kind !== "emoji") || a.kind !== b.kind) throw new Error("not the same layer");
      expect(Math.abs(b.x - a.x)).toBeLessThan(0.5);
      expect(Math.abs(b.y - a.y)).toBeLessThan(0.5);
      expect(Math.abs(b.size - a.size)).toBeLessThan(0.5);
    }
  });
});
