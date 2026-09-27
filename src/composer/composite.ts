/**
 * Shows a design on the folder. The folder's pixels are the template layers Rust rendered
 * (`composer_template`): coverage masks of the back and front panels, the paper and back rim
 * between them, and the front's rims on top. Stacking the design between them here gives the
 * same picture Rust makes when it saves the icon (the Rust tests check that the layers add up
 * to its render), and it's cheap enough to redo on every frame of a drag.
 *
 * On the canvas the folder or drive comes apart into its parts (base.ts), so the user can give one
 * a colour of its own, leave it out, or move and turn the whole of it: [`drawBase`] stacks the
 * pieces the same way, and draws the saved icon too when anything about the folder was changed.
 */
import type { ComposerPiece } from "../lib/tauri";
import { CANVAS, isHome, type BaseEdit, type Shape } from "./doc";
import { frameMatrix, loadPieces, partColor, recoloured, type Piece } from "./base";
import { cssColor } from "./color";
import type { Point } from "./geometry";
// Not the composer's own catalog: the browser preview (lib/devMock.ts) draws with this too, and
// that catalog comes with the composer's chunk.
import { t } from "../i18n";

export type TemplateImages = {
  back: CanvasImageSource;
  front: CanvasImageSource;
  middle: CanvasImageSource;
  top: CanvasImageSource;
  outline: CanvasImageSource;
  /** Edge of the template's pictures, in pixels: what the pieces' rectangles are measured in. */
  size: number;
  /** The same folder or drive taken apart, bottom first (base.ts); empty when it came without them. */
  pieces: Piece[];
};

export type TemplateSources = { back: string; front: string; middle: string; top: string; outline: string; size?: number; pieces?: ComposerPiece[] };

function load(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error(t("common.errors.templateLoad")));
    img.src = src;
  });
}

export async function loadTemplate(src: TemplateSources): Promise<TemplateImages> {
  const [[back, front, middle, top, outline], pieces] = await Promise.all([
    Promise.all([src.back, src.front, src.middle, src.top, src.outline].map(load)),
    loadPieces(src.pieces, load),
  ]);
  return { back, front, middle, top, outline, size: src.size ?? front.naturalWidth, pieces };
}

/** Each drive's face colour once it has been worked out, by its layers and its face's box. */
const faceColors = new WeakMap<TemplateImages, Map<string, string | null>>();

/**
 * The colour a drive's face shows with nothing on it: the plain drive (`middle`) over `face`, a
 * box in canvas units, averaged. New words and shapes are dark on a light face, as they are on a
 * light colour. Null when the picture can't be read.
 */
export function faceColor(images: TemplateImages, face: [number, number, number, number]): string | null {
  const key = face.join();
  const known = faceColors.get(images) ?? new Map<string, string | null>();
  faceColors.set(images, known);
  if (known.has(key)) return known.get(key) ?? null;
  let color: string | null = null;
  try {
    const n = 24;
    const canvas = document.createElement("canvas");
    canvas.width = n;
    canvas.height = n;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    const src = images.middle as HTMLImageElement;
    const k = (src.naturalWidth || Number(src.width)) / 1024;
    if (ctx && k > 0) {
      const [x0, y0, x1, y1] = face;
      ctx.drawImage(src, x0 * k, y0 * k, (x1 - x0) * k, (y1 - y0) * k, 0, 0, n, n);
      const px = ctx.getImageData(0, 0, n, n).data;
      let [r, g, b, a] = [0, 0, 0, 0];
      for (let i = 0; i < px.length; i += 4) {
        const w = px[i + 3] / 255;
        r += px[i] * w;
        g += px[i + 1] * w;
        b += px[i + 2] * w;
        a += w;
      }
      const hex = (v: number) => Math.round(v / a).toString(16).padStart(2, "0");
      if (a > 0) color = `#${hex(r)}${hex(g)}${hex(b)}`;
    }
  } catch {
    color = null;
  }
  known.set(key, color);
  return color;
}

type Ctx = CanvasRenderingContext2D;

function reset(c: Ctx) {
  c.setTransform(1, 0, 0, 1, 0, 0);
  c.globalAlpha = 1;
  c.globalCompositeOperation = "source-over";
  c.shadowColor = "transparent";
}

/** `source` cut to one of the template's masks, on `scratch`. */
function masked(scratch: HTMLCanvasElement, source: CanvasImageSource, mask: CanvasImageSource, px: number): HTMLCanvasElement {
  const s = scratch.getContext("2d")!;
  reset(s);
  s.clearRect(0, 0, px, px);
  s.drawImage(source, 0, 0, px, px);
  s.globalCompositeOperation = "destination-in";
  s.drawImage(mask, 0, 0, px, px);
  s.globalCompositeOperation = "source-over";
  return scratch;
}

/** The design on the folder: back panel, paper and rim, front panel, front rims. */
export function drawOnFolder(ctx: Ctx, design: CanvasImageSource, t: TemplateImages, px: number, scratch: HTMLCanvasElement, alpha = 1) {
  if (scratch.width !== px || scratch.height !== px) {
    scratch.width = px;
    scratch.height = px;
  }
  ctx.save();
  reset(ctx);
  ctx.globalAlpha = alpha;
  ctx.drawImage(masked(scratch, design, t.back, px), 0, 0, px, px);
  ctx.drawImage(t.middle, 0, 0, px, px);
  ctx.drawImage(masked(scratch, design, t.front, px), 0, 0, px, px);
  ctx.drawImage(t.top, 0, 0, px, px);
  ctx.restore();
}

/** The folder's edges as a line in `color`. */
function drawOutline(ctx: Ctx, t: TemplateImages, px: number, scratch: HTMLCanvasElement, color: string, alpha: number) {
  if (scratch.width !== px || scratch.height !== px) {
    scratch.width = px;
    scratch.height = px;
  }
  const s = scratch.getContext("2d")!;
  reset(s);
  s.clearRect(0, 0, px, px);
  s.drawImage(t.outline, 0, 0, px, px);
  s.globalCompositeOperation = "source-in";
  s.fillStyle = color;
  s.fillRect(0, 0, px, px);
  s.globalCompositeOperation = "source-over";
  ctx.save();
  reset(ctx);
  ctx.globalAlpha = alpha;
  ctx.drawImage(scratch, 0, 0, px, px);
  ctx.restore();
}

/** How the editor shows what's see-through: a fine checkerboard, in greys that sit on any backdrop. */
const CHECK_CELLS = 40;
const CHECK_LIGHT = "rgba(120, 128, 140, 0.10)";
const CHECK_DARK = "rgba(120, 128, 140, 0.22)";
/** The folder's edges where the design leaves it see-through. */
const SKELETON_LINE = "rgba(110, 118, 130, 0.6)";

let checker: HTMLCanvasElement | null = null;
function checkerboard(px: number): HTMLCanvasElement {
  if (!checker) checker = document.createElement("canvas");
  if (checker.width !== px) {
    checker.width = px;
    checker.height = px;
    const g = checker.getContext("2d")!;
    const cell = px / CHECK_CELLS;
    g.fillStyle = CHECK_LIGHT;
    g.fillRect(0, 0, px, px);
    g.fillStyle = CHECK_DARK;
    for (let y = 0; y < CHECK_CELLS; y++) {
      for (let x = y % 2; x < CHECK_CELLS; x += 2) g.fillRect(Math.round(x * cell), Math.round(y * cell), Math.ceil(cell), Math.ceil(cell));
    }
  }
  return checker;
}

/**
 * The folder as it is with nothing on it, for the editor only: a checkerboard over its whole
 * shape (back panel with its tab, and front) and its edges as a fine grey line. A design drawn
 * over it hides both where it's solid, so an empty or see-through design still shows exactly the
 * folder it will be, never a bare sheet of paper with a rim floating under it.
 */
function drawSkeleton(ctx: Ctx, t: TemplateImages, px: number, scratch: HTMLCanvasElement) {
  if (scratch.width !== px || scratch.height !== px) {
    scratch.width = px;
    scratch.height = px;
  }
  const s = scratch.getContext("2d")!;
  reset(s);
  s.clearRect(0, 0, px, px);
  s.drawImage(t.back, 0, 0, px, px);
  s.drawImage(t.front, 0, 0, px, px);
  s.globalCompositeOperation = "source-in";
  s.drawImage(checkerboard(px), 0, 0);
  s.globalCompositeOperation = "source-over";
  ctx.save();
  reset(ctx);
  ctx.drawImage(scratch, 0, 0, px, px);
  ctx.restore();
  drawOutline(ctx, t, px, scratch, SKELETON_LINE, 1);
}

/** The design on the folder as the editor shows it: the folder's skeleton under whatever of it is see-through. */
export function drawFolderView(ctx: Ctx, design: CanvasImageSource, t: TemplateImages, px: number, scratch: HTMLCanvasElement) {
  drawSkeleton(ctx, t, px, scratch);
  drawOnFolder(ctx, design, t, px, scratch);
}

// ---------- the folder or drive in its parts ----------

/** Spare canvases for drawing a design on its folder or drive in parts, kept by whoever draws often. */
export type Scratch = { mask: HTMLCanvasElement; icon: HTMLCanvasElement };

export const makeScratch = (): Scratch => ({ mask: document.createElement("canvas"), icon: document.createElement("canvas") });

function sized(c: HTMLCanvasElement, px: number): HTMLCanvasElement {
  if (c.width !== px || c.height !== px) {
    c.width = px;
    c.height = px;
  }
  return c;
}

/** How a design goes on its folder or drive, besides the design itself. */
export type Onto = {
  /** What was changed of the folder or drive. */
  base: BaseEdit | undefined;
  /**
   * On a drive, whose face is a label the design is printed on: leaving the label out leaves the
   * design where it is. A folder's panels are what the design is printed on, and take it with them.
   */
  drive: boolean;
  /** What the frame turns and sizes the folder or drive about, in canvas units: its middle. */
  pivot: Point;
  /** The design without its backgrounds (the colour and pattern layers at its bottom), for a part the design shows on that has a colour of its own; see [`needsUpper`]. */
  upper?: CanvasImageSource | null;
  /** A part to tint in `color`: the one selected, on the stage. */
  highlight?: { part: string; color: string } | null;
};

const isGone = (base: BaseEdit | undefined, part: string) => base?.parts?.[part]?.hidden === true || base?.parts?.[part]?.removed === true;

/**
 * Whether a part the design shows on has a colour of its own, which takes the place of the
 * design's backgrounds there: then the design is needed without them too ([`Onto`]'s `upper`).
 */
export function needsUpper(t: TemplateImages, base: BaseEdit | undefined): boolean {
  return t.pieces.some((p) => p.role === "surface" && base?.parts?.[p.part]?.color !== undefined && !isGone(base, p.part));
}

/** `source` (a picture or a colour) cut to one piece, on `scratch`, with nothing anywhere else. */
function cutToPiece(scratch: HTMLCanvasElement, source: CanvasImageSource | string, piece: Piece, k: number, px: number): HTMLCanvasElement {
  const s = scratch.getContext("2d")!;
  reset(s);
  s.clearRect(0, 0, px, px);
  const [x, y, w, h] = piece.rect.map((v) => v * k);
  if (typeof source === "string") {
    s.fillStyle = cssColor(source);
    s.fillRect(x, y, w, h);
  } else s.drawImage(source, 0, 0, px, px);
  s.globalCompositeOperation = "destination-in";
  s.drawImage(piece.img, x, y, w, h);
  s.globalCompositeOperation = "source-over";
  // Whatever a web view makes of the canvas outside the piece, nothing shows there.
  const [x0, y0, x1, y1] = [Math.max(0, Math.floor(x)), Math.max(0, Math.floor(y)), Math.min(px, Math.ceil(x + w)), Math.min(px, Math.ceil(y + h))];
  s.clearRect(0, 0, px, y0);
  s.clearRect(0, y1, px, px - y1);
  s.clearRect(0, y0, x0, y1 - y0);
  s.clearRect(x1, y0, px - x1, y1 - y0);
  return scratch;
}

/**
 * The design on its folder or drive, drawn from the pieces: each part in the colour it was given
 * (its light and shade kept), as see-through as it was made, or left out; where the design shows,
 * the design cut to it, or on a part with a colour of its own that colour under the design's
 * upper layers. With `skeleton`, the checkerboard under whatever of it is see-through, and its
 * edges while no part is left out. All of it moved, turned and sized by the frame. What the stage
 * shows and, without the skeleton and the tint, the icon that's saved.
 */
export function drawBase(ctx: Ctx, design: CanvasImageSource, t: TemplateImages, px: number, scratch: Scratch, onto: Onto, skeleton: boolean) {
  const frame = onto.base?.frame;
  const moved = frame !== undefined && !isHome(frame);
  const target = moved ? sized(scratch.icon, px) : null;
  const g = target ? target.getContext("2d")! : ctx;
  const mask = sized(scratch.mask, px);
  g.save();
  reset(g);
  if (target) g.clearRect(0, 0, px, px);
  const k = px / t.size;
  const edit = (part: string) => onto.base?.parts?.[part] ?? {};
  const gone = (part: string) => isGone(onto.base, part);
  const shows = (p: Piece) => p.role === "surface" && (onto.drive || !gone(p.part));
  const place = (p: Piece) => p.rect.map((v) => v * k) as [number, number, number, number];

  if (skeleton) {
    const m = mask.getContext("2d")!;
    reset(m);
    m.clearRect(0, 0, px, px);
    for (const p of t.pieces) if (shows(p)) m.drawImage(p.img, ...place(p));
    m.globalCompositeOperation = "source-in";
    m.drawImage(checkerboard(px), 0, 0);
    m.globalCompositeOperation = "source-over";
    g.drawImage(mask, 0, 0, px, px);
    // The folder's whole outline, while the whole folder is there.
    if (!t.pieces.some((p) => gone(p.part))) drawOutline(g, t, px, mask, SKELETON_LINE, 1);
    reset(g);
  }

  /** The selected part's pieces tinted, each where it's drawn so what's over it still hides it. */
  const tint = (pieces: Piece[]) => {
    if (!onto.highlight || pieces.length === 0) return;
    const m = mask.getContext("2d")!;
    reset(m);
    m.clearRect(0, 0, px, px);
    for (const p of pieces) m.drawImage(p.img, ...place(p));
    m.globalCompositeOperation = "source-in";
    m.fillStyle = onto.highlight.color;
    m.fillRect(0, 0, px, px);
    m.globalCompositeOperation = "source-over";
    g.drawImage(mask, 0, 0, px, px);
  };
  const tinted = (p: Piece) => p.part === onto.highlight?.part && p.role !== "light";

  for (const p of t.pieces) {
    const e = edit(p.part);
    if (p.role === "surface") {
      if (!shows(p)) continue;
      const own = !gone(p.part) && e.color ? e.color : null;
      g.globalAlpha = onto.drive ? 1 : (e.opacity ?? 1);
      // A folder's panel in a colour of its own: the colour, then the design's upper layers over it.
      if (own && !onto.drive) g.drawImage(cutToPiece(mask, own, p, k, px), 0, 0, px, px);
      g.drawImage(cutToPiece(mask, own ? (onto.upper ?? design) : design, p, k, px), 0, 0, px, px);
      g.globalAlpha = 1;
      if (tinted(p) && !gone(p.part)) tint([p]);
      continue;
    }
    if (gone(p.part)) continue;
    g.globalAlpha = e.opacity ?? 1;
    const lum = e.color && p.role === "paint" ? partColor(t.pieces, p.part)?.lum : undefined;
    g.drawImage(lum !== undefined && e.color ? recoloured(p, e.color, lum) : p.img, ...place(p));
    g.globalAlpha = 1;
    if (tinted(p)) tint([p]);
  }
  // A part left out still shows where it would be while it's selected, over everything.
  if (onto.highlight && gone(onto.highlight.part)) tint(t.pieces.filter(tinted));
  g.restore();

  if (target && frame) {
    ctx.save();
    reset(ctx);
    const m = frameMatrix(frame, onto.pivot);
    const u = px / CANVAS;
    ctx.setTransform(m[0], m[1], m[2], m[3], m[4] * u, m[5] * u);
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(target, 0, 0, px, px);
    ctx.restore();
  }
}

/** A free icon as the editor shows it: the checkerboard under whatever of the square is see-through. */
export function drawFreeView(ctx: Ctx, design: CanvasImageSource, px: number) {
  ctx.save();
  reset(ctx);
  ctx.drawImage(checkerboard(px), 0, 0, px, px);
  ctx.drawImage(design, 0, 0, px, px);
  ctx.restore();
}

let grey: HTMLCanvasElement | null = null;
/** A flat grey the size of a design, for the folder shown behind a free icon. */
function greyDesign(px: number): HTMLCanvasElement {
  if (!grey) grey = document.createElement("canvas");
  if (grey.width !== px) {
    grey.width = px;
    grey.height = px;
    const g = grey.getContext("2d")!;
    g.fillStyle = "#c9ced6";
    g.fillRect(0, 0, px, px);
  }
  return grey;
}

export type View = {
  shape: Shape;
  /** The folder skeleton switch: on, the design is seen on the folder (or, for a free icon, beside a faint one). */
  skeleton: boolean;
  /** The colour the folder's edges are drawn in when the design is shown flat. */
  guide: string;
  /** The folder or drive in its parts, as changed; left out, it's drawn whole as FolderSkin draws it. */
  onto?: Onto | null;
};

/**
 * The stage's picture of a design: on the folder (or the drive) with the skeleton on, flat with
 * its edges drawn over it with the skeleton off. A free icon is the design itself, with a faint
 * folder (or the drive it came from) behind it for scale while the skeleton is on.
 */
export function drawView(ctx: Ctx, design: HTMLCanvasElement, t: TemplateImages | null, px: number, view: View, scratch: HTMLCanvasElement, scratchSet?: Scratch) {
  ctx.save();
  reset(ctx);
  ctx.clearRect(0, 0, px, px);
  ctx.restore();
  if (!t) {
    ctx.save();
    reset(ctx);
    ctx.drawImage(design, 0, 0, px, px);
    ctx.restore();
    return;
  }
  if (view.shape === "folder" || view.shape === "drive") {
    if (view.skeleton && view.onto && t.pieces.length > 0 && scratchSet) drawBase(ctx, design, t, px, scratchSet, view.onto, true);
    else if (view.skeleton) drawFolderView(ctx, design, t, px, scratch);
    else {
      ctx.save();
      reset(ctx);
      ctx.drawImage(design, 0, 0, px, px);
      ctx.restore();
      drawOutline(ctx, t, px, scratch, view.guide, 0.9);
    }
    return;
  }
  if (view.skeleton) {
    drawOnFolder(ctx, greyDesign(px), t, px, scratch, 0.32);
    ctx.save();
    reset(ctx);
    ctx.drawImage(design, 0, 0, px, px);
    ctx.restore();
    return;
  }
  drawFreeView(ctx, design, px);
}
