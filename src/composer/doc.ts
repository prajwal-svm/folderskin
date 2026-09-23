/**
 * The composer's design document: a stack of layers drawn bottom to top onto the icon's
 * 1024-unit canvas, the same canvas the Rust folder template is measured in. What sits at
 * (x, y) here lands at (x, y) on the folder.
 *
 * Everything in this file is pure: the document is plain data (it is saved beside the skin as
 * JSON and read back to edit it again), and every change makes a new document.
 */
import { normalizeColor } from "./color";
import { centreOf, type Parts } from "./parts";

export { centreOf, FALLBACK_PARTS, type Parts } from "./parts";

/** Edge of the design canvas, in the units every position and size below is measured in. */
export const CANVAS = 1024;
export const DOC_VERSION = 1;
/** More layers than anyone needs for an icon; also what a document read from disk is cut to. */
export const MAX_LAYERS = 64;
export const MAX_TEXT = 400;
/** A picture layer's data URL, at most. A 2048 px PNG with transparency is well under this. */
const MAX_SRC = 24_000_000;

/** "folder": painted onto FolderSkin's folder. "free": the design is the whole icon. */
export type Shape = "folder" | "free";

export const BLENDS = [
  { id: "normal", label: "Normal" },
  { id: "multiply", label: "Multiply" },
  { id: "screen", label: "Screen" },
  { id: "overlay", label: "Overlay" },
  { id: "soft-light", label: "Soft light" },
  { id: "hard-light", label: "Hard light" },
  { id: "color-dodge", label: "Dodge" },
  { id: "color-burn", label: "Burn" },
  { id: "darken", label: "Darken" },
  { id: "lighten", label: "Lighten" },
  { id: "difference", label: "Difference" },
  { id: "exclusion", label: "Exclusion" },
  { id: "hue", label: "Hue" },
  { id: "saturation", label: "Saturation" },
  { id: "color", label: "Colour" },
  { id: "luminosity", label: "Luminosity" },
] as const;
export type Blend = (typeof BLENDS)[number]["id"];

export type Stop = { at: number; color: string };
/** A flat colour or a gradient across the layer's own box. `angle` follows CSS: 180 is top to bottom. */
export type Paint =
  | { type: "solid"; color: string }
  | { type: "linear"; angle: number; stops: Stop[] }
  | { type: "radial"; cx: number; cy: number; stops: Stop[] };

/** A soft shadow, or with no offset a glow. Blur and offsets in canvas units. */
export type Shadow = { color: string; blur: number; x: number; y: number };
/** A line around a layer's silhouette: a sticker's white edge, or text's outline. */
export type Edge = { color: string; width: number };

export const PATTERNS = [
  { id: "stripes", label: "Stripes" },
  { id: "dots", label: "Polka dots" },
  { id: "checks", label: "Checks" },
  { id: "gingham", label: "Gingham" },
  { id: "grid", label: "Grid" },
  { id: "waves", label: "Waves" },
  { id: "zigzag", label: "Zigzag" },
  { id: "halftone", label: "Halftone" },
  { id: "confetti", label: "Confetti" },
  { id: "grain", label: "Grain" },
] as const;
export type PatternKind = (typeof PATTERNS)[number]["id"];

export const SHAPES = [
  { id: "rect", label: "Rectangle" },
  { id: "ellipse", label: "Circle" },
  { id: "ring", label: "Ring" },
  { id: "triangle", label: "Triangle" },
  { id: "diamond", label: "Diamond" },
  { id: "star", label: "Star" },
  { id: "burst", label: "Burst" },
  { id: "polygon", label: "Polygon" },
  { id: "heart", label: "Heart" },
  { id: "arrow", label: "Arrow" },
  { id: "bar", label: "Bar" },
  { id: "banner", label: "Banner" },
  { id: "bubble", label: "Bubble" },
] as const;
export type ShapeKind = (typeof SHAPES)[number]["id"];

export type Align = "left" | "center" | "right";

/** Picture adjustments. All 0 is the picture as it is. */
export type ImageFx = {
  /** -100 to 100 */
  brightness: number;
  /** -100 to 100 */
  contrast: number;
  /** -100 to 100 */
  saturation: number;
  /** -180 to 180 degrees */
  hue: number;
  /** 0 to 40 canvas units */
  blur: number;
  /** 0 to 100 */
  grayscale: number;
  /** 0 to 100 */
  sepia: number;
  /** 0 to 100 */
  invert: number;
};

type Common = {
  id: string;
  /** A name the user gave it in the layers list. */
  name?: string;
  hidden?: boolean;
  /** Can't be picked or moved on the canvas; the layers list still selects it. */
  locked?: boolean;
  /** 0 to 1 */
  opacity: number;
  blend: Blend;
};

/** A layer with a place on the canvas: its centre, turn and mirror, and its effects. */
type Placed = {
  x: number;
  y: number;
  /** Degrees, clockwise. */
  rotation: number;
  flipX: boolean;
  flipY: boolean;
  shadow: Shadow | null;
  edge: Edge | null;
};

/** Covers the whole canvas: a colour or a gradient. */
export type FillLayer = Common & { kind: "fill"; paint: Paint };
/** Covers the whole canvas: a repeating pattern. `background` may be transparent. */
export type PatternLayer = Common & {
  kind: "pattern";
  pattern: PatternKind;
  color: string;
  background: string;
  /** The size of one repeat, in canvas units. */
  scale: number;
  angle: number;
  /** Where confetti and grain fall; shuffling picks another. */
  seed: number;
};
export type TextLayer = Common &
  Placed & {
    kind: "text";
    text: string;
    /** A font id from fonts.ts, or `custom:<family>`. */
    font: string;
    weight: number;
    italic: boolean;
    /** Font size in canvas units. */
    size: number;
    paint: Paint;
    align: Align;
    /** Line height as a multiple of the size. */
    lineHeight: number;
    /** Extra space between letters, as a share of the size. */
    spacing: number;
    upper: boolean;
    /** -100 (a smile) to 100 (an arch); 0 is straight. */
    curve: number;
    stroke: Edge | null;
  };
export type EmojiLayer = Common & Placed & { kind: "emoji"; char: string; size: number };
export type ShapeLayer = Common &
  Placed & {
    kind: "shape";
    shape: ShapeKind;
    w: number;
    h: number;
    paint: Paint;
    stroke: Edge | null;
    /** Rounding of a rectangle's, banner's or bubble's corners, 0 to 1. */
    radius: number;
    /** Points of a star or burst, sides of a polygon. */
    points: number;
    /** A star's inner radius or a ring's hole, as a share of the outer. */
    inner: number;
  };
/** A picture that fills its box, cropped around its middle if the box is another shape. */
export type ImageLayer = Common &
  Placed & {
    kind: "image";
    src: string;
    /** The picture's own size in pixels. */
    iw: number;
    ih: number;
    w: number;
    h: number;
    /** Corner rounding, 0 to 1. */
    radius: number;
    fx: ImageFx;
  };

/**
 * How an icon sits on the folder. "emboss" presses it into the folder the way macOS draws the
 * symbol on a folder: a shade of the folder's own colour, with a lit lower lip and a shadowed top
 * edge. "flat" paints it in one colour; "original" keeps the colour its pack gives it (a brand's).
 */
export type IconLook = "emboss" | "flat" | "original";
export const ICON_LOOKS: { id: IconLook; label: string }[] = [
  { id: "emboss", label: "Pressed in" },
  { id: "flat", label: "Flat" },
  { id: "original", label: "Original" },
];

/**
 * An icon from an icon pack. Its drawing (path data in its pack's square grid) is kept in the
 * layer, so a design opens again even after the pack is removed from this computer.
 */
export type IconLayer = Common &
  Placed & {
    kind: "icon";
    /** The pack and the icon's name in it, to name the layer and find the icon again. */
    pack: string;
    icon: string;
    paths: string[];
    /** Line icons: which of the paths are filled shapes rather than lines. */
    filled: number[];
    style: "stroke" | "fill";
    /** The side of the square grid the paths are drawn in: 24 for most packs. */
    viewBox: number;
    /** A line icon's line width, in grid units. */
    strokeWidth: number;
    evenOdd: boolean;
    /** The side of its box on the canvas. */
    size: number;
    look: IconLook;
    /** The flat look's colour, and the pressed-in look's when it doesn't follow the folder. */
    paint: Paint;
    /** The pressed-in look takes its colour from the folder's own, as macOS does. */
    auto: boolean;
    /** How deep the pressed-in look is, 0 to 100. */
    depth: number;
    /** The colour the pack gives this icon, such as a brand's, for the original look. */
    brand?: string;
  };

export type Layer = FillLayer | PatternLayer | TextLayer | EmojiLayer | ShapeLayer | ImageLayer | IconLayer;
export type PlacedLayer = TextLayer | EmojiLayer | ShapeLayer | ImageLayer | IconLayer;
export type LayerKind = Layer["kind"];

export type Doc = { version: 1; shape: Shape; layers: Layer[] };

// ---------- making layers ----------

let seq = 0;
/** A short id, unique within a session and very likely across them. */
export function newId(): string {
  seq += 1;
  return `l${Date.now().toString(36).slice(-4)}${seq.toString(36)}${Math.random().toString(36).slice(2, 6)}`;
}

export const NO_FX: ImageFx = { brightness: 0, contrast: 0, saturation: 0, hue: 0, blur: 0, grayscale: 0, sepia: 0, invert: 0 };

const common = (): Common => ({ id: newId(), opacity: 1, blend: "normal" });
const placed = (x: number, y: number): Placed => ({ x, y, rotation: 0, flipX: false, flipY: false, shadow: null, edge: null });

export const solid = (color: string): Paint => ({ type: "solid", color });
export const linear = (angle: number, ...colors: string[]): Paint => ({
  type: "linear",
  angle,
  stops: colors.map((color, i) => ({ at: colors.length === 1 ? 0 : i / (colors.length - 1), color })),
});
export const radial = (cx: number, cy: number, ...colors: string[]): Paint => ({
  type: "radial",
  cx,
  cy,
  stops: colors.map((color, i) => ({ at: colors.length === 1 ? 0 : i / (colors.length - 1), color })),
});

export function makeFill(paint: Paint): FillLayer {
  return { ...common(), kind: "fill", paint };
}

export function makePattern(pattern: PatternKind, color = "#ffffff59", background = "#00000000"): PatternLayer {
  const scale = pattern === "grain" ? 2 : pattern === "confetti" ? 150 : pattern === "halftone" ? 36 : pattern === "dots" ? 76 : 64;
  const angle = pattern === "stripes" || pattern === "halftone" ? 45 : 0;
  return { ...common(), kind: "pattern", pattern, color, background, scale, angle, seed: 1 + Math.floor(Math.random() * 9999) };
}

export function makeText(text: string, x: number, y: number, color: string): TextLayer {
  return {
    ...common(),
    ...placed(x, y),
    kind: "text",
    text,
    font: "rounded",
    weight: 800,
    italic: false,
    size: 150,
    paint: solid(color),
    align: "center",
    lineHeight: 1.1,
    spacing: 0,
    upper: false,
    curve: 0,
    stroke: null,
  };
}

export function makeEmoji(char: string, x: number, y: number): EmojiLayer {
  return { ...common(), ...placed(x, y), kind: "emoji", char, size: 420 };
}

export function makeShape(shape: ShapeKind, x: number, y: number, color: string): ShapeLayer {
  const wide = shape === "bar" || shape === "banner" || shape === "arrow";
  return {
    ...common(),
    ...placed(x, y),
    kind: "shape",
    shape,
    w: wide ? 620 : 380,
    h: shape === "bar" ? 90 : wide ? 190 : shape === "bubble" ? 320 : 380,
    paint: solid(color),
    stroke: null,
    radius: shape === "rect" ? 0.18 : shape === "bubble" ? 0.5 : shape === "bar" ? 1 : 0,
    points: shape === "burst" ? 18 : shape === "polygon" ? 6 : 5,
    inner: shape === "burst" ? 0.8 : shape === "ring" ? 0.62 : 0.45,
  };
}

export function makeImage(src: string, iw: number, ih: number, box: { x: number; y: number; w: number; h: number }): ImageLayer {
  return { ...common(), ...placed(box.x, box.y), kind: "image", src, iw, ih, w: box.w, h: box.h, radius: 0, fx: { ...NO_FX } };
}

/** An icon from a pack as a pack stores it. */
export type IconDrawing = {
  pack: string;
  icon: string;
  paths: string[];
  filled?: number[];
  style: "stroke" | "fill";
  viewBox: number;
  strokeWidth: number;
  evenOdd?: boolean;
  brand?: string;
};

/** A new icon layer, pressed into the folder unless `look` says otherwise. */
export function makeIcon(drawing: IconDrawing, x: number, y: number, look: IconLook = "emboss", color = "#ffffff"): IconLayer {
  return {
    ...common(),
    ...placed(x, y),
    kind: "icon",
    pack: drawing.pack,
    icon: drawing.icon,
    paths: [...drawing.paths],
    filled: [...(drawing.filled ?? [])],
    style: drawing.style,
    viewBox: drawing.viewBox,
    strokeWidth: drawing.strokeWidth,
    evenOdd: drawing.evenOdd === true,
    size: 340,
    look: look === "original" && !drawing.brand ? "flat" : look,
    paint: solid(color),
    auto: true,
    depth: 60,
    ...(drawing.brand ? { brand: drawing.brand } : {}),
  };
}

/** The icon's name as people say it: "arrow-big-up" is "Arrow big up". */
export function iconName(icon: string): string {
  const words = icon.replace(/[-_]+/g, " ").trim();
  return words ? words.charAt(0).toUpperCase() + words.slice(1) : "Icon";
}

/** Where a picture goes: covering the whole folder, or a picture with its own shape (a logo, a cut-out) sitting on the front. */
export function imageBox(iw: number, ih: number, parts: Parts, cover: boolean): { x: number; y: number; w: number; h: number } {
  const aspect = iw / Math.max(1, ih);
  if (cover) {
    const [x0, y0, x1, y1] = parts.folder;
    const fw = x1 - x0;
    const fh = y1 - y0;
    const w = Math.max(fw, fh * aspect);
    const h = w / aspect;
    return { x: (x0 + x1) / 2, y: (y0 + y1) / 2, w, h };
  }
  const c = centreOf(parts.front);
  const room = 0.62 * Math.min(parts.front[2] - parts.front[0], parts.front[3] - parts.front[1]);
  const w = aspect >= 1 ? room : room * aspect;
  return { x: c.x, y: c.y, w, h: w / aspect };
}

// ---------- reading a layer ----------

export const isPlaced = (layer: Layer): layer is PlacedLayer =>
  layer.kind === "text" || layer.kind === "emoji" || layer.kind === "shape" || layer.kind === "image" || layer.kind === "icon";

/** Whether a layer covers the whole canvas (it has no box of its own). */
export const isCovering = (layer: Layer): layer is FillLayer | PatternLayer => layer.kind === "fill" || layer.kind === "pattern";

export const patternLabel = (id: PatternKind) => PATTERNS.find((p) => p.id === id)?.label ?? "Pattern";
export const shapeLabel = (id: ShapeKind) => SHAPES.find((s) => s.id === id)?.label ?? "Shape";

/** What the layers list calls a layer. */
export function layerLabel(layer: Layer, index = 1): string {
  if (layer.name) return layer.name;
  switch (layer.kind) {
    case "fill":
      return index === 0 ? "Background" : layer.paint.type === "solid" ? "Colour" : "Gradient";
    case "pattern":
      return patternLabel(layer.pattern);
    case "text": {
      const line = layer.text.split("\n").find((l) => l.trim()) ?? "";
      const words = Array.from(line.trim());
      return words.length === 0 ? "Text" : words.length > 22 ? `${words.slice(0, 21).join("")}…` : words.join("");
    }
    case "emoji":
      return `${layer.char} Emoji`;
    case "shape":
      return shapeLabel(layer.shape);
    case "image":
      return "Picture";
    case "icon":
      return iconName(layer.icon);
  }
}

/**
 * What a layer shows on the folder, when that's words: a text layer's first line. The layers
 * list shows it beside a name the user gave the layer, so the two are never mistaken for each
 * other.
 */
export function layerContent(layer: Layer): string | null {
  if (layer.kind !== "text") return null;
  const line = layer.text.split("\n").find((l) => l.trim())?.trim() ?? "";
  return line || null;
}

/** The colour a paint mostly shows: a solid colour, or a gradient's middle stop. */
export function mainColor(paint: Paint): string {
  if (paint.type === "solid") return paint.color;
  return paint.stops[Math.floor(paint.stops.length / 2)]?.color ?? "#000000";
}

/** The colour at the bottom of the design, which new text and shapes are made to stand out from. */
export function backgroundColor(doc: Doc): string | null {
  const base = doc.layers.find((l) => !l.hidden && (l.kind === "fill" || l.kind === "image"));
  if (!base) return null;
  if (base.kind === "fill") return mainColor(base.paint);
  return null;
}

/** A name for a design nobody has named: its first words, or its emoji. */
export function suggestName(doc: Doc): string | null {
  for (let i = doc.layers.length - 1; i >= 0; i--) {
    const l = doc.layers[i];
    if (l.kind === "text" && !l.hidden) {
      const line = l.text.split("\n").map((s) => s.trim()).find(Boolean);
      if (line) return Array.from(line).slice(0, 40).join("");
    }
  }
  return null;
}

// ---------- changing the document ----------

export function emptyDoc(shape: Shape = "folder"): Doc {
  return { version: DOC_VERSION, shape, layers: [] };
}

export const indexOf = (doc: Doc, id: string) => doc.layers.findIndex((l) => l.id === id);
export const findLayer = (doc: Doc, id: string | null) => (id ? (doc.layers.find((l) => l.id === id) ?? null) : null);

/** Adds `layer` at `index` (on top by default). */
export function addLayer(doc: Doc, layer: Layer, index = doc.layers.length): Doc {
  if (doc.layers.length >= MAX_LAYERS) return doc;
  const layers = [...doc.layers];
  layers.splice(Math.max(0, Math.min(index, layers.length)), 0, layer);
  return { ...doc, layers };
}

/** Just above the bottom run of full-canvas layers: where a new pattern or background goes. */
export function coveringTop(doc: Doc): number {
  let i = 0;
  while (i < doc.layers.length && isCovering(doc.layers[i])) i++;
  return i;
}

export function mapLayer(doc: Doc, id: string, fn: (layer: Layer) => Layer): Doc {
  let changed = false;
  const layers = doc.layers.map((l) => {
    if (l.id !== id) return l;
    const next = fn(l);
    if (next !== l) changed = true;
    return next;
  });
  return changed ? { ...doc, layers } : doc;
}

/** Sets some fields of one layer. Fields its kind doesn't have are ignored by the renderer. */
export function patchLayer(doc: Doc, id: string, patch: Record<string, unknown>): Doc {
  return mapLayer(doc, id, (l) => ({ ...l, ...patch }) as Layer);
}

export function removeLayer(doc: Doc, id: string): Doc {
  const layers = doc.layers.filter((l) => l.id !== id);
  return layers.length === doc.layers.length ? doc : { ...doc, layers };
}

/** A copy of a layer just above it, nudged so both can be seen. Returns the copy's id. */
export function duplicateLayer(doc: Doc, id: string): { doc: Doc; id: string | null } {
  const i = indexOf(doc, id);
  if (i < 0 || doc.layers.length >= MAX_LAYERS) return { doc, id: null };
  const source = doc.layers[i];
  const copy = cloneLayer(source, isPlaced(source) ? 28 : 0);
  return { doc: addLayer(doc, copy, i + 1), id: copy.id };
}

/** A deep copy with a new id, moved by `offset` canvas units if it has a place. */
export function cloneLayer<L extends Layer>(layer: L, offset = 0): L {
  const copy = JSON.parse(JSON.stringify(layer)) as L;
  copy.id = newId();
  if (isPlaced(copy) && offset) {
    copy.x += offset;
    copy.y += offset;
  }
  return copy;
}

/** Moves a layer to `to` in the stack (0 is the bottom). */
export function moveLayer(doc: Doc, id: string, to: number): Doc {
  const from = indexOf(doc, id);
  const target = Math.max(0, Math.min(to, doc.layers.length - 1));
  if (from < 0 || from === target) return doc;
  const layers = [...doc.layers];
  const [layer] = layers.splice(from, 1);
  layers.splice(target, 0, layer);
  return { ...doc, layers };
}

export const bringForward = (doc: Doc, id: string) => moveLayer(doc, id, indexOf(doc, id) + 1);
export const sendBackward = (doc: Doc, id: string) => moveLayer(doc, id, indexOf(doc, id) - 1);
export const bringToFront = (doc: Doc, id: string) => moveLayer(doc, id, doc.layers.length - 1);
export const sendToBack = (doc: Doc, id: string) => moveLayer(doc, id, 0);

// ---------- reading a document back ----------

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const num = (v: unknown, fallback: number, lo = -Infinity, hi = Infinity) =>
  typeof v === "number" && Number.isFinite(v) ? Math.min(hi, Math.max(lo, v)) : fallback;
const bool = (v: unknown) => v === true;
function oneOf<T extends string>(v: unknown, ids: readonly T[], fallback: T): T {
  return typeof v === "string" && (ids as readonly string[]).includes(v) ? (v as T) : fallback;
}
const BLEND_IDS = BLENDS.map((b) => b.id);
const PATTERN_IDS = PATTERNS.map((p) => p.id);
const SHAPE_IDS = SHAPES.map((s) => s.id);
const text = (v: unknown, fallback: string, max: number) => (typeof v === "string" ? Array.from(v).slice(0, max).join("") : fallback);

function readStops(v: unknown): Stop[] {
  if (!Array.isArray(v)) return [];
  return v
    .filter(isObj)
    .slice(0, 8)
    .map((s) => ({ at: num(s.at, 0, 0, 1), color: normalizeColor(s.color, "#000000") }))
    .sort((a, b) => a.at - b.at);
}

export function readPaint(v: unknown, fallback: Paint): Paint {
  if (!isObj(v)) return fallback;
  if (v.type === "solid") return { type: "solid", color: normalizeColor(v.color, "#000000") };
  const stops = readStops(v.stops);
  if (stops.length < 2) return fallback;
  if (v.type === "linear") return { type: "linear", angle: num(v.angle, 180, -360, 360), stops };
  if (v.type === "radial") return { type: "radial", cx: num(v.cx, 0.5, 0, 1), cy: num(v.cy, 0.5, 0, 1), stops };
  return fallback;
}

function readShadow(v: unknown): Shadow | null {
  if (!isObj(v)) return null;
  return { color: normalizeColor(v.color, "#00000066"), blur: num(v.blur, 20, 0, 200), x: num(v.x, 0, -300, 300), y: num(v.y, 12, -300, 300) };
}

function readEdge(v: unknown): Edge | null {
  if (!isObj(v)) return null;
  return { color: normalizeColor(v.color, "#ffffff"), width: num(v.width, 12, 0, 120) };
}

function readFx(v: unknown): ImageFx {
  const o = isObj(v) ? v : {};
  return {
    brightness: num(o.brightness, 0, -100, 100),
    contrast: num(o.contrast, 0, -100, 100),
    saturation: num(o.saturation, 0, -100, 100),
    hue: num(o.hue, 0, -180, 180),
    blur: num(o.blur, 0, 0, 40),
    grayscale: num(o.grayscale, 0, 0, 100),
    sepia: num(o.sepia, 0, 0, 100),
    invert: num(o.invert, 0, 0, 100),
  };
}

const POS = 4 * CANVAS;

function readLayer(v: unknown): Layer | null {
  if (!isObj(v)) return null;
  const base: Common = {
    id: typeof v.id === "string" && /^[A-Za-z0-9_-]{1,40}$/.test(v.id) ? v.id : newId(),
    opacity: num(v.opacity, 1, 0, 1),
    blend: oneOf(v.blend, BLEND_IDS, "normal"),
  };
  if (typeof v.name === "string" && v.name.trim()) base.name = text(v.name, "", 40);
  if (v.hidden === true) base.hidden = true;
  if (v.locked === true) base.locked = true;
  const where = (): Placed => ({
    x: num(v.x, CANVAS / 2, -POS, POS),
    y: num(v.y, CANVAS / 2, -POS, POS),
    rotation: num(v.rotation, 0, -3600, 3600),
    flipX: bool(v.flipX),
    flipY: bool(v.flipY),
    shadow: readShadow(v.shadow),
    edge: readEdge(v.edge),
  });
  switch (v.kind) {
    case "fill":
      return { ...base, kind: "fill", paint: readPaint(v.paint, solid("#3a86ff")) };
    case "pattern":
      return {
        ...base,
        kind: "pattern",
        pattern: oneOf(v.pattern, PATTERN_IDS, "stripes"),
        color: normalizeColor(v.color, "#ffffff"),
        background: normalizeColor(v.background, "#00000000"),
        scale: num(v.scale, 64, 1, 600),
        angle: num(v.angle, 0, -360, 360),
        seed: Math.round(num(v.seed, 1, 0, 1e9)),
      };
    case "text": {
      const t = text(v.text, "", MAX_TEXT);
      return {
        ...base,
        ...where(),
        kind: "text",
        text: t,
        font: text(v.font, "rounded", 120),
        weight: Math.round(num(v.weight, 700, 100, 900) / 100) * 100,
        italic: bool(v.italic),
        size: num(v.size, 150, 4, 2000),
        paint: readPaint(v.paint, solid("#ffffff")),
        align: oneOf(v.align, ["left", "center", "right"] as const, "center"),
        lineHeight: num(v.lineHeight, 1.1, 0.6, 3),
        spacing: num(v.spacing, 0, -0.3, 1.5),
        upper: bool(v.upper),
        curve: num(v.curve, 0, -100, 100),
        stroke: readEdge(v.stroke),
      };
    }
    case "emoji": {
      const char = text(v.char, "", 16).trim();
      if (!char) return null;
      return { ...base, ...where(), kind: "emoji", char, size: num(v.size, 420, 4, 3000) };
    }
    case "shape":
      return {
        ...base,
        ...where(),
        kind: "shape",
        shape: oneOf(v.shape, SHAPE_IDS, "rect"),
        w: num(v.w, 380, 1, POS),
        h: num(v.h, 380, 1, POS),
        paint: readPaint(v.paint, solid("#3a86ff")),
        stroke: readEdge(v.stroke),
        radius: num(v.radius, 0, 0, 1),
        points: Math.round(num(v.points, 5, 3, 40)),
        inner: num(v.inner, 0.45, 0.05, 0.95),
      };
    case "image": {
      const src = typeof v.src === "string" ? v.src : "";
      if (!/^data:image\/(png|jpeg|webp|gif);base64,/.test(src) || src.length > MAX_SRC) return null;
      return {
        ...base,
        ...where(),
        kind: "image",
        src,
        iw: num(v.iw, 1, 1, 100_000),
        ih: num(v.ih, 1, 1, 100_000),
        w: num(v.w, 400, 1, POS),
        h: num(v.h, 400, 1, POS),
        radius: num(v.radius, 0, 0, 1),
        fx: readFx(v.fx),
      };
    }
    case "icon": {
      const paths = Array.isArray(v.paths) ? v.paths.filter((p): p is string => typeof p === "string" && PATH_DATA.test(p) && p.length <= MAX_PATH) : [];
      if (paths.length === 0 || paths.length > MAX_PATHS || paths.join("").length > MAX_ICON) return null;
      const viewBox = num(v.viewBox, 24, 1, 1024);
      const filled = Array.isArray(v.filled) ? v.filled.filter((i): i is number => Number.isInteger(i) && i >= 0 && i < paths.length) : [];
      const brand = typeof v.brand === "string" ? normalizeColor(v.brand, "") : "";
      return {
        ...base,
        ...where(),
        kind: "icon",
        pack: text(v.pack, "icons", 40),
        icon: text(v.icon, "icon", 80),
        paths,
        filled: [...new Set(filled)],
        style: v.style === "fill" ? "fill" : "stroke",
        viewBox,
        strokeWidth: num(v.strokeWidth, 2, 0, viewBox / 4),
        evenOdd: bool(v.evenOdd),
        size: num(v.size, 340, 4, 3000),
        look: oneOf(v.look, ICON_LOOK_IDS, "emboss"),
        paint: readPaint(v.paint, solid("#ffffff")),
        auto: v.auto !== false,
        depth: num(v.depth, 60, 0, 100),
        ...(brand ? { brand } : {}),
      };
    }
    default:
      return null;
  }
}

/** Path data and nothing else: commands, numbers, separators. */
const PATH_DATA = /^[MmLlHhVvCcSsQqTtAaZz0-9eE.,+\-\s]+$/;
const MAX_PATH = 40_000;
const MAX_PATHS = 64;
const MAX_ICON = 120_000;
const ICON_LOOK_IDS = ICON_LOOKS.map((l) => l.id);

/**
 * A design read from disk or storage, made safe to draw: unknown layers are dropped, numbers
 * are kept to sensible ranges, colours made canonical and repeated ids replaced. `null` when it
 * isn't a design at all.
 */
export function parseDoc(value: unknown): Doc | null {
  if (!isObj(value) || !Array.isArray(value.layers)) return null;
  if (typeof value.version === "number" && value.version > DOC_VERSION) return null;
  const seen = new Set<string>();
  const layers: Layer[] = [];
  for (const raw of value.layers.slice(0, MAX_LAYERS)) {
    const layer = readLayer(raw);
    if (!layer) continue;
    if (seen.has(layer.id)) layer.id = newId();
    seen.add(layer.id);
    layers.push(layer);
  }
  return { version: DOC_VERSION, shape: value.shape === "free" ? "free" : "folder", layers };
}
