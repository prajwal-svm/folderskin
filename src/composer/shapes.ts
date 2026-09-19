/**
 * The outlines of the composer's shapes, traced in a shape layer's own frame (centred on 0,0,
 * `w` by `h`). They trace onto anything with the canvas path methods: a 2D context when
 * drawing, a recorder in the tests. These are the user's shapes; the folder's own outline is
 * never drawn here (it comes from Rust).
 */
import type { ShapeLayer } from "./doc";

export type PathSink = {
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
  arcTo(x1: number, y1: number, x2: number, y2: number, r: number): void;
  ellipse(x: number, y: number, rx: number, ry: number, rotation: number, start: number, end: number, ccw?: boolean): void;
  closePath(): void;
};

type P = [number, number];

/** Vertices of a star (or burst) with `points` tips, starting at the top, going clockwise. */
export function starPoints(w: number, h: number, points: number, inner: number): P[] {
  const out: P[] = [];
  const n = Math.max(3, Math.round(points));
  for (let i = 0; i < n * 2; i++) {
    const a = -Math.PI / 2 + (i * Math.PI) / n;
    const k = i % 2 === 0 ? 1 : inner;
    out.push([(Math.cos(a) * w * k) / 2, (Math.sin(a) * h * k) / 2]);
  }
  return out;
}

/** Vertices of a regular polygon with `sides` sides, a corner at the top. */
export function polygonPoints(w: number, h: number, sides: number): P[] {
  const n = Math.max(3, Math.round(sides));
  return Array.from({ length: n }, (_, i) => {
    const a = -Math.PI / 2 + (i * 2 * Math.PI) / n;
    return [(Math.cos(a) * w) / 2, (Math.sin(a) * h) / 2] as P;
  });
}

/** The classic parametric heart, sampled and fitted to a unit box centred on 0,0. */
const HEART: P[] = (() => {
  const raw: P[] = [];
  for (let i = 0; i < 72; i++) {
    const t = (i / 72) * Math.PI * 2;
    raw.push([16 * Math.sin(t) ** 3, -(13 * Math.cos(t) - 5 * Math.cos(2 * t) - 2 * Math.cos(3 * t) - Math.cos(4 * t))]);
  }
  const xs = raw.map((p) => p[0]);
  const ys = raw.map((p) => p[1]);
  const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  return raw.map(([x, y]) => [(x - x0) / (x1 - x0) - 0.5, (y - y0) / (y1 - y0) - 0.5] as P);
})();

export function heartPoints(w: number, h: number): P[] {
  return HEART.map(([x, y]) => [x * w, y * h] as P);
}

function poly(sink: PathSink, pts: P[]) {
  pts.forEach(([x, y], i) => (i === 0 ? sink.moveTo(x, y) : sink.lineTo(x, y)));
  sink.closePath();
}

/** A rounded rectangle with the same corner on every side (clamped to fit). */
export function roundRect(sink: PathSink, x: number, y: number, w: number, h: number, r: number) {
  const rr = Math.max(0, Math.min(r, w / 2, h / 2));
  sink.moveTo(x + rr, y);
  sink.lineTo(x + w - rr, y);
  sink.arcTo(x + w, y, x + w, y + rr, rr);
  sink.lineTo(x + w, y + h - rr);
  sink.arcTo(x + w, y + h, x + w - rr, y + h, rr);
  sink.lineTo(x + rr, y + h);
  sink.arcTo(x, y + h, x, y + h - rr, rr);
  sink.lineTo(x, y + rr);
  sink.arcTo(x, y, x + rr, y, rr);
  sink.closePath();
}

/** Whether a shape is filled with the even-odd rule (a ring's hole). */
export const evenOdd = (layer: Pick<ShapeLayer, "shape">) => layer.shape === "ring";

export function traceShape(sink: PathSink, layer: Pick<ShapeLayer, "shape" | "w" | "h" | "radius" | "points" | "inner">) {
  const { w, h } = layer;
  const x = -w / 2;
  const y = -h / 2;
  switch (layer.shape) {
    case "rect":
      return roundRect(sink, x, y, w, h, (layer.radius * Math.min(w, h)) / 2);
    case "ellipse":
      sink.moveTo(w / 2, 0);
      sink.ellipse(0, 0, w / 2, h / 2, 0, 0, Math.PI * 2);
      return sink.closePath();
    case "ring":
      sink.moveTo(w / 2, 0);
      sink.ellipse(0, 0, w / 2, h / 2, 0, 0, Math.PI * 2);
      sink.closePath();
      sink.moveTo((w / 2) * layer.inner, 0);
      sink.ellipse(0, 0, (w / 2) * layer.inner, (h / 2) * layer.inner, 0, 0, Math.PI * 2, true);
      return sink.closePath();
    case "triangle":
      return poly(sink, [
        [0, y],
        [w / 2, h / 2],
        [x, h / 2],
      ]);
    case "diamond":
      return poly(sink, [
        [0, y],
        [w / 2, 0],
        [0, h / 2],
        [x, 0],
      ]);
    case "star":
    case "burst":
      return poly(sink, starPoints(w, h, layer.points, layer.inner));
    case "polygon":
      return poly(sink, polygonPoints(w, h, layer.points));
    case "heart":
      return poly(sink, heartPoints(w, h));
    case "arrow": {
      const head = Math.min(h * 0.95, w * 0.45);
      const shaft = h * 0.42;
      return poly(sink, [
        [x, -shaft / 2],
        [w / 2 - head, -shaft / 2],
        [w / 2 - head, y],
        [w / 2, 0],
        [w / 2 - head, h / 2],
        [w / 2 - head, shaft / 2],
        [x, shaft / 2],
      ]);
    }
    case "bar":
      return roundRect(sink, x, y, w, h, (layer.radius * Math.min(w, h)) / 2);
    case "banner": {
      const notch = Math.min(h * 0.38, w * 0.2);
      return poly(sink, [
        [x, y],
        [w / 2, y],
        [w / 2 - notch, 0],
        [w / 2, h / 2],
        [x, h / 2],
        [x + notch, 0],
      ]);
    }
    case "bubble": {
      const tail = h * 0.22;
      const bh = h - tail;
      const r = Math.max(0, Math.min((layer.radius * Math.min(w, bh)) / 2, w / 2, bh / 2));
      const bottom = y + bh;
      const tx = x + w * 0.22;
      sink.moveTo(x + r, y);
      sink.lineTo(w / 2 - r, y);
      sink.arcTo(w / 2, y, w / 2, y + r, r);
      sink.lineTo(w / 2, bottom - r);
      sink.arcTo(w / 2, bottom, w / 2 - r, bottom, r);
      sink.lineTo(Math.max(tx + w * 0.16, x + r), bottom);
      sink.lineTo(tx - w * 0.04, h / 2);
      sink.lineTo(tx, bottom);
      sink.lineTo(x + r, bottom);
      sink.arcTo(x, bottom, x, bottom - r, r);
      sink.lineTo(x, y + r);
      sink.arcTo(x, y, x + r, y, r);
      return sink.closePath();
    }
  }
}
