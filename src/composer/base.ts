/**
 * The folder or drive under a design, taken apart into the parts the user can change on the
 * canvas: a folder's tab, back, paper and front, a drive's case, face, port and the rest. Rust
 * draws each part on its own (`template_pieces_in`, `drive::pieces`), and stacked back they are
 * the folder or drive it draws whole, so the canvas shows the saved icon (docs/COMPOSER.md).
 *
 * A piece is one part at one place in the stack: a *surface* is where the design shows, *paint*
 * the part's own colours and *light* the light and shade over a part, which keep their colour
 * whatever colour the part is given. This file reads them: which parts there are, what colour
 * each is, a part in another colour, which part is under the pointer, and the frame that moves,
 * turns and sizes the whole folder or drive. composite.ts draws them.
 */
import "../i18n/composer";
import { t, type MessageKey } from "../i18n";
import type { ComposerPiece } from "../lib/tauri";
import { parseColor, toHex } from "./color";
import { CANVAS, type Frame, type PartEdit } from "./doc";
import type { Box, Point } from "./geometry";

export type PieceRole = ComposerPiece["role"];

/** One of a template's pieces with its picture loaded. `rect` is in the template's own pixels. */
export type Piece = { part: string; role: PieceRole; rect: [number, number, number, number]; img: HTMLImageElement };

/** Loads every piece's picture with `load`, in their order. */
export function loadPieces(list: ComposerPiece[] | undefined, load: (src: string) => Promise<HTMLImageElement>): Promise<Piece[]> {
  return Promise.all((list ?? []).map(async (p) => ({ part: p.part, role: p.role, rect: p.rect, img: await load(p.src) })));
}

// ---------- the parts ----------

/** One of the folder's or drive's parts, as the layers list shows it. */
export type BasePart = {
  id: string;
  /** The design shows on it: a folder's tab, back and front, a drive's face. */
  surface: boolean;
  /** It has colours of its own, which a colour of its own changes. */
  paints: boolean;
};

/** The parts in `pieces`, top first, each where it shows highest in the stack: its light and shade don't count. */
export function partsOf(pieces: Piece[]): BasePart[] {
  const at = new Map<string, { index: number; light: boolean }>();
  pieces.forEach((p, index) => {
    const light = p.role === "light";
    const was = at.get(p.part);
    if (!was || (was.light && !light) || (was.light === light && index > was.index)) at.set(p.part, { index, light });
  });
  return [...at.entries()]
    .sort((a, b) => b[1].index - a[1].index)
    .map(([id]) => ({
      id,
      surface: pieces.some((p) => p.part === id && p.role === "surface"),
      paints: pieces.some((p) => p.part === id && p.role === "paint"),
    }));
}

/** What a part is called in the layers list: "Case", "Port holes". */
export function partLabel(id: string): string {
  const key = `composer.base.parts.${id}`;
  const name = t(key as MessageKey);
  return name === key ? id.charAt(0).toUpperCase() + id.slice(1).replace(/-/g, " ") : name;
}

/** A part's own colour: the colour its paint shows on average, and how light that is (0 to 255). */
export type PartColor = { color: string; lum: number };

const lumOf = (r: number, g: number, b: number) => 0.2126 * r + 0.7152 * g + 0.0722 * b;

const partColors = new WeakMap<Piece[], Map<string, PartColor | null>>();

/**
 * The colour `part` shows, from its paint, each pixel counted as much as it covers; null for a
 * part with none (a folder's panels take theirs from the design) or when it can't be read.
 */
export function partColor(pieces: Piece[], part: string): PartColor | null {
  const known = partColors.get(pieces) ?? new Map<string, PartColor | null>();
  partColors.set(pieces, known);
  if (known.has(part)) return known.get(part) ?? null;
  let color: PartColor | null = null;
  try {
    let [r, g, b, weight] = [0, 0, 0, 0];
    for (const p of pieces) {
      if (p.part !== part || p.role !== "paint") continue;
      const [, , w, h] = p.rect;
      const k = Math.min(1, 48 / Math.max(w, h, 1));
      const [sw, sh] = [Math.max(1, Math.round(w * k)), Math.max(1, Math.round(h * k))];
      const c = document.createElement("canvas");
      c.width = sw;
      c.height = sh;
      const ctx = c.getContext("2d", { willReadFrequently: true });
      if (!ctx) continue;
      ctx.drawImage(p.img, 0, 0, sw, sh);
      const px = ctx.getImageData(0, 0, sw, sh).data;
      // Each pixel of the small copy stands for this much of the piece.
      const share = (w * h) / (sw * sh);
      for (let i = 0; i < px.length; i += 4) {
        const a = (px[i + 3] / 255) * share;
        r += px[i] * a;
        g += px[i + 1] * a;
        b += px[i + 2] * a;
        weight += a;
      }
    }
    if (weight > 0) {
      const [R, G, B] = [r / weight, g / weight, b / weight];
      color = { color: toHex({ r: R, g: G, b: B, a: 1 }), lum: lumOf(R, G, B) };
    }
  } catch {
    color = null;
  }
  known.set(part, color);
  return color;
}

const tinted = new WeakMap<Piece, { color: string; canvas: HTMLCanvasElement }>();

/**
 * A paint piece in `color`, its light and shade kept: every pixel moves as far from `color` as it
 * was from the part's own colour (`lum`, how light that is). A lit edge stays lighter than the
 * rest, a shaded one darker, whatever the colour. Kept until the piece is asked for in another
 * colour, so drawing it again while the colour stays is free.
 */
export function recoloured(piece: Piece, color: string, lum: number): CanvasImageSource {
  const c = parseColor(color);
  if (!c) return piece.img;
  const kept = tinted.get(piece);
  if (kept?.color === color) return kept.canvas;
  const [w, h] = [piece.img.naturalWidth, piece.img.naturalHeight];
  const canvas = kept?.canvas ?? document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const g = canvas.getContext("2d", { willReadFrequently: true });
  if (!g || w === 0 || h === 0) return piece.img;
  g.clearRect(0, 0, w, h);
  g.drawImage(piece.img, 0, 0);
  const data = g.getImageData(0, 0, w, h);
  const px = data.data;
  for (let i = 0; i < px.length; i += 4) {
    if (px[i + 3] === 0) continue;
    const d = lumOf(px[i], px[i + 1], px[i + 2]) - lum;
    px[i] = c.r + d;
    px[i + 1] = c.g + d;
    px[i + 2] = c.b + d;
  }
  g.putImageData(data, 0, 0);
  tinted.set(piece, { color, canvas });
  return canvas;
}

// ---------- which part is where ----------

let probe: CanvasRenderingContext2D | null = null;

/** How much of `piece` covers the template pixel `(x, y)`, 0 to 255. */
function alphaAt(piece: Piece, x: number, y: number): number {
  const [px, py, w, h] = piece.rect;
  const [ix, iy] = [Math.floor(x - px), Math.floor(y - py)];
  if (ix < 0 || iy < 0 || ix >= w || iy >= h) return 0;
  if (!probe) {
    const c = document.createElement("canvas");
    c.width = 1;
    c.height = 1;
    probe = c.getContext("2d", { willReadFrequently: true });
  }
  if (!probe) return 0;
  probe.clearRect(0, 0, 1, 1);
  probe.drawImage(piece.img, ix, iy, 1, 1, 0, 0, 1, 1);
  return probe.getImageData(0, 0, 1, 1).data[3];
}

/** How much of a piece has to be under the pointer for it to be the part picked there. */
const PICK_ALPHA = 64;

/**
 * The part that shows at `p` (canvas units, on the folder or drive before its frame moves it),
 * and whether the design shows there: the top piece there that isn't light and shade and isn't
 * left out. A drive's face shows its design even with the face's own colours left out. Null off
 * the folder or drive.
 */
export function partAt(pieces: Piece[], size: number, parts: Record<string, PartEdit> | undefined, drive: boolean, p: Point): { part: string; surface: boolean } | null {
  const k = size / CANVAS;
  for (let i = pieces.length - 1; i >= 0; i--) {
    const piece = pieces[i];
    if (piece.role === "light") continue;
    const e = parts?.[piece.part];
    const gone = e?.hidden === true || e?.removed === true;
    if (gone && !(drive && piece.role === "surface")) continue;
    if (alphaAt(piece, p.x * k, p.y * k) >= PICK_ALPHA) return { part: piece.part, surface: piece.role === "surface" };
  }
  return null;
}

// ---------- the frame ----------

/** A 2D transform as a canvas takes it, `[a, b, c, d, e, f]`: x' = a·x + c·y + e, y' = b·x + d·y + f. */
export type Matrix = [number, number, number, number, number, number];

/** The frame as a transform of canvas units: turned and sized about `pivot`, then moved. */
export function frameMatrix(f: Frame, pivot: Point): Matrix {
  const r = (f.rotation * Math.PI) / 180;
  const [cos, sin] = [Math.cos(r) * f.scale, Math.sin(r) * f.scale];
  return [cos, sin, -sin, cos, pivot.x + f.x - (cos * pivot.x - sin * pivot.y), pivot.y + f.y - (sin * pivot.x + cos * pivot.y)];
}

export const applyMatrix = (m: Matrix, p: Point): Point => ({ x: m[0] * p.x + m[2] * p.y + m[4], y: m[1] * p.x + m[3] * p.y + m[5] });

export function invertMatrix(m: Matrix): Matrix {
  const [a, b, c, d, e, f] = m;
  const det = a * d - b * c || 1;
  return [d / det, -b / det, -c / det, a / det, (c * f - d * e) / det, (b * e - a * f) / det];
}

/** A box on the folder or drive, where the frame puts it on the canvas: moved, turned and sized with it. */
export function framedBox(b: Box, f: Frame, pivot: Point): Box {
  const c = applyMatrix(frameMatrix(f, pivot), b);
  return { x: c.x, y: c.y, w: b.w * f.scale, h: b.h * f.scale, rotation: b.rotation + f.rotation };
}

// ---------- what's selected ----------

/** The selection when it's the whole folder or drive. */
export const BASE_ID = "@base";
/** The selection when it's one of the folder's or drive's parts. */
export const partSelection = (part: string) => `${BASE_ID}:${part}`;

/**
 * What a selection is when it's the folder or drive: `{ part: null }` for the whole of it,
 * `{ part: "case" }` for a part. Null for a layer or nothing. A layer's id never has an `@`.
 */
export function baseSelection(id: string | null): { part: string | null } | null {
  if (id === BASE_ID) return { part: null };
  if (id?.startsWith(`${BASE_ID}:`)) return { part: id.slice(BASE_ID.length + 1) };
  return null;
}
