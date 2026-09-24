/**
 * Moving, sizing and turning layers on the composer's canvas, and finding what the pointer is
 * over. Pure maths in canvas units; the stage converts to and from screen pixels.
 */

export type Point = { x: number; y: number };
/** A layer's box: its centre, size and turn (degrees, clockwise). */
export type Box = { x: number; y: number; w: number; h: number; rotation: number };
/** An axis-aligned rectangle. */
export type Rect = { x0: number; y0: number; x1: number; y1: number };

export type Handle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "rot";
export const CORNERS: Handle[] = ["nw", "ne", "se", "sw"];
export const ALL_SIDES: Handle[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];

/** Which way each handle points, in the box's own frame. */
const DIR: Record<Exclude<Handle, "rot">, [number, number]> = {
  nw: [-1, -1],
  n: [0, -1],
  ne: [1, -1],
  e: [1, 0],
  se: [1, 1],
  s: [0, 1],
  sw: [-1, 1],
  w: [-1, 0],
};

/** The smallest a layer can be sized to, in canvas units. */
export const MIN_SIZE = 8;

const rad = (deg: number) => (deg * Math.PI) / 180;

/** A point in the box's own frame (its centre at 0,0, unturned). */
export function toLocal(p: Point, box: Box): Point {
  const t = rad(box.rotation);
  const dx = p.x - box.x;
  const dy = p.y - box.y;
  return { x: dx * Math.cos(t) + dy * Math.sin(t), y: -dx * Math.sin(t) + dy * Math.cos(t) };
}

/** A point in the box's own frame, back on the canvas. */
export function toWorld(p: Point, box: Box): Point {
  const t = rad(box.rotation);
  return { x: box.x + p.x * Math.cos(t) - p.y * Math.sin(t), y: box.y + p.x * Math.sin(t) + p.y * Math.cos(t) };
}

export function contains(box: Box, p: Point, pad = 0): boolean {
  const l = toLocal(p, box);
  return Math.abs(l.x) <= box.w / 2 + pad && Math.abs(l.y) <= box.h / 2 + pad;
}

/** The axis-aligned rectangle a turned box covers. */
export function bounds(box: Box, pad = 0): Rect {
  const t = rad(box.rotation);
  const ex = Math.abs((box.w / 2) * Math.cos(t)) + Math.abs((box.h / 2) * Math.sin(t)) + pad;
  const ey = Math.abs((box.w / 2) * Math.sin(t)) + Math.abs((box.h / 2) * Math.cos(t)) + pad;
  return { x0: box.x - ex, y0: box.y - ey, x1: box.x + ex, y1: box.y + ey };
}

/** Where a handle sits on the canvas. The turn handle floats `rotGap` above the top edge. */
export function handlePoint(box: Box, handle: Handle, rotGap = 0): Point {
  if (handle === "rot") return toWorld({ x: 0, y: -box.h / 2 - rotGap }, box);
  const [sx, sy] = DIR[handle];
  return toWorld({ x: (sx * box.w) / 2, y: (sy * box.h) / 2 }, box);
}

/** The handle within `radius` of `p`, nearest first; the turn handle wins ties. */
export function hitHandle(box: Box, p: Point, handles: Handle[], radius: number, rotGap: number): Handle | null {
  let best: Handle | null = null;
  let bestD = radius;
  for (const h of handles) {
    const q = handlePoint(box, h, rotGap);
    const d = Math.hypot(q.x - p.x, q.y - p.y);
    if (d <= bestD || (h === "rot" && d <= radius)) {
      best = h;
      bestD = d;
    }
  }
  return best;
}

/** The resize cursor that matches a handle's direction once the box is turned. */
export function cursorFor(handle: Handle, rotation: number): string {
  if (handle === "rot") return "grab";
  const [sx, sy] = DIR[handle];
  const angle = ((Math.atan2(-sy, sx) * 180) / Math.PI - rotation + 360) % 180;
  if (angle < 22.5 || angle >= 157.5) return "ew-resize";
  if (angle < 67.5) return "nesw-resize";
  if (angle < 112.5) return "ns-resize";
  return "nwse-resize";
}

/**
 * The box after dragging `handle` to `p` (canvas point). The opposite side stays put, or the
 * centre with `fromCenter`. With `keepAspect` the box scales evenly: always from a corner, and
 * from a side when the layer can't be stretched (text, emoji).
 */
export function resizeBox(start: Box, handle: Exclude<Handle, "rot">, p: Point, opts: { keepAspect: boolean; fromCenter: boolean }): Box {
  const [sx, sy] = DIR[handle];
  const l = toLocal(p, start);
  const ax = opts.fromCenter ? 0 : (-sx * start.w) / 2;
  const ay = opts.fromCenter ? 0 : (-sy * start.h) / 2;
  const span = (v: number, a: number) => (opts.fromCenter ? 2 * Math.abs(v) : Math.abs(v - a));
  let w = sx !== 0 ? Math.max(MIN_SIZE, span(l.x, ax)) : start.w;
  let h = sy !== 0 ? Math.max(MIN_SIZE, span(l.y, ay)) : start.h;
  if (opts.keepAspect) {
    const k =
      sx !== 0 && sy !== 0 ? Math.max(w / start.w, h / start.h) : sx !== 0 ? w / start.w : h / start.h;
    const min = MIN_SIZE / Math.min(start.w, start.h);
    const s = Math.max(k, min);
    w = start.w * s;
    h = start.h * s;
  }
  const cx = opts.fromCenter ? 0 : sx !== 0 ? ax + (sx * w) / 2 : 0;
  const cy = opts.fromCenter ? 0 : sy !== 0 ? ay + (sy * h) / 2 : 0;
  const c = toWorld({ x: cx, y: cy }, start);
  return { x: c.x, y: c.y, w, h, rotation: start.rotation };
}

/** Normalises degrees to -180 (exclusive) to 180. */
export function normAngle(deg: number): number {
  const a = ((deg % 360) + 360) % 360;
  return a > 180 ? a - 360 : a;
}

/**
 * The turn after dragging the turn handle from `from` to `to` around the box's centre. Snaps to
 * 15° steps with `step`, and otherwise to the nearest quarter turn within 4°.
 */
export function rotateBy(start: Box, from: Point, to: Point, step: boolean): number {
  const a0 = Math.atan2(from.y - start.y, from.x - start.x);
  const a1 = Math.atan2(to.y - start.y, to.x - start.x);
  let r = start.rotation + ((a1 - a0) * 180) / Math.PI;
  if (step) r = Math.round(r / 15) * 15;
  else {
    const quarter = Math.round(r / 90) * 90;
    if (Math.abs(r - quarter) < 4) r = quarter;
  }
  return normAngle(r);
}

export type Guide = { axis: "x" | "y"; at: number };
export type Targets = { xs: number[]; ys: number[] };

/**
 * Snaps a moving box to the nearest target line within `threshold`: its left edge, centre or
 * right edge to a vertical line, and likewise for the horizontal. Returns the correction and the
 * lines it snapped to, for the stage to draw as guides.
 */
export function snap(box: Box, targets: Targets, threshold: number): { dx: number; dy: number; guides: Guide[] } {
  const b = bounds(box);
  const best = (edges: number[], lines: number[]) => {
    let d = Infinity;
    let at = 0;
    for (const e of edges)
      for (const t of lines)
        if (Math.abs(t - e) < Math.abs(d)) {
          d = t - e;
          at = t;
        }
    return Math.abs(d) <= threshold ? { d, at } : null;
  };
  const sx = best([b.x0, box.x, b.x1], targets.xs);
  const sy = best([b.y0, box.y, b.y1], targets.ys);
  const guides: Guide[] = [];
  if (sx) guides.push({ axis: "x", at: sx.at });
  if (sy) guides.push({ axis: "y", at: sy.at });
  return { dx: sx?.d ?? 0, dy: sy?.d ?? 0, guides };
}

/** The lines other boxes offer to snap to: their edges and centres. */
export function boxTargets(boxes: Box[]): Targets {
  const xs: number[] = [];
  const ys: number[] = [];
  for (const box of boxes) {
    const b = bounds(box);
    xs.push(b.x0, box.x, b.x1);
    ys.push(b.y0, box.y, b.y1);
  }
  return { xs, ys };
}

/** Room kept between what's already on the folder and something new beside it. */
const SPOT_GAP = 28;
/** And between something new and the front panel's edges. */
const SPOT_MARGIN = 24;
/** The middle first, then beside it, above and below, then the corners. */
const AROUND = [
  [0, 0],
  [-1, 0],
  [1, 0],
  [0, -1],
  [0, 1],
  [-1, -1],
  [1, -1],
  [-1, 1],
  [1, 1],
];

/**
 * Where something `w` × `h` can go on the front panel (`front`, x0 y0 x1 y1) without covering
 * any of `taken`: the middle when it's free, otherwise a whole step beside, above or below it,
 * the step clearing the biggest thing already there with a gap. `null` when nowhere is free.
 */
export function freeSpot(front: [number, number, number, number], centre: Point, w: number, h: number, taken: Box[]): Point | null {
  const [fx0, fy0, fx1, fy1] = front;
  const widest = taken.reduce((m, b) => Math.max(m, b.w), w);
  const tallest = taken.reduce((m, b) => Math.max(m, b.h), h);
  const fits = (x: number, y: number) => x - w / 2 >= fx0 + SPOT_MARGIN && x + w / 2 <= fx1 - SPOT_MARGIN && y - h / 2 >= fy0 + SPOT_MARGIN && y + h / 2 <= fy1 - SPOT_MARGIN;
  const free = (x: number, y: number) => taken.every((b) => Math.abs(b.x - x) * 2 >= b.w + w || Math.abs(b.y - y) * 2 >= b.h + h);
  for (const [dx, dy] of AROUND) {
    const x = centre.x + dx * (widest / 2 + w / 2 + SPOT_GAP);
    const y = centre.y + dy * (tallest / 2 + h / 2 + SPOT_GAP);
    if (fits(x, y) && free(x, y)) return { x, y };
  }
  return null;
}

/** An icon's size: the first one big, the ones after it smaller. */
export const FIRST_ICON = 340;
export const NEXT_ICON = 240;
/** How small an icon may be drawn to find it a free spot, rather than on top of another. */
const SMALLEST_ICON = 160;

/**
 * Where the next icon goes, and how big: in a free spot on the front panel at its usual size, or
 * a little smaller where the panel is too narrow for that (Windows' front is narrower than the
 * Mac's), and only when nothing is free anywhere, stepped down and across from the middle, one
 * step per icon already there (`count`).
 */
export function placeIcon(front: [number, number, number, number], centre: Point, taken: Box[], count: number): Point & { size: number } {
  const usual = count === 0 ? FIRST_ICON : NEXT_ICON;
  for (let size = usual; size >= SMALLEST_ICON; size -= 20) {
    const at = freeSpot(front, centre, size, size, taken);
    if (at) return { ...at, size };
  }
  return { x: centre.x + (count % 5) * 36, y: centre.y + (count % 5) * 36, size: usual };
}
