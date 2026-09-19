/**
 * Pattern layers: repeating stripes, dots, checks and the rest, drawn over the whole canvas in
 * canvas units. Confetti and grain are random, but seeded, so a design draws the same every time.
 */
import { cssColor, hsvToRgb, parseColor, rgbToHsv, toHex } from "./color";
import { CANVAS, type PatternLayer } from "./doc";

/** A small, fast seeded random number generator (mulberry32): the same seed, the same numbers. */
export function seeded(seed: number): () => number {
  let a = seed >>> 0 || 1;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Confetti's colours: the layer's colour first, then four more around the colour wheel from it. */
export function confettiColors(color: string): string[] {
  const c = parseColor(color) ?? { r: 255, g: 90, b: 95, a: 1 };
  const hsv = rgbToHsv(c);
  const lively = { h: hsv.h, s: Math.max(0.55, hsv.s), v: Math.max(0.85, hsv.v) };
  return [toHex(c), ...[72, 144, 216, 288].map((d) => toHex(hsvToRgb({ ...lively, h: lively.h + d }, c.a)))];
}

/** Draws on a 2D context whose transform maps canvas units to pixels. */
type Ctx = CanvasRenderingContext2D;

/** Half the diagonal of the canvas: a turned pattern still covers every corner. */
const REACH = Math.ceil((CANVAS * Math.SQRT2) / 2) + 4;

export function drawPattern(ctx: Ctx, layer: PatternLayer, grainTile: (seed: number, color: string) => CanvasImageSource) {
  const bg = parseColor(layer.background);
  if (bg && bg.a > 0) {
    ctx.fillStyle = cssColor(layer.background);
    ctx.fillRect(0, 0, CANVAS, CANVAS);
  }
  const p = Math.max(2, layer.scale);
  const color = cssColor(layer.color);

  if (layer.pattern === "grain") {
    // A tile of noise, repeated; `scale` is the size of one speck.
    const tile = grainTile(layer.seed, layer.color);
    const side = 128 * Math.max(0.5, layer.scale);
    ctx.save();
    ctx.imageSmoothingEnabled = layer.scale < 1.5;
    for (let y = 0; y < CANVAS; y += side) for (let x = 0; x < CANVAS; x += side) ctx.drawImage(tile, x, y, side, side);
    ctx.restore();
    return;
  }

  ctx.save();
  ctx.translate(CANVAS / 2, CANVAS / 2);
  ctx.rotate((layer.angle * Math.PI) / 180);
  ctx.fillStyle = color;
  ctx.strokeStyle = color;
  const R = REACH;
  const from = -Math.ceil(R / p) * p;

  switch (layer.pattern) {
    case "stripes":
      for (let x = from; x < R; x += p) ctx.fillRect(x, -R, p / 2, 2 * R);
      break;
    case "dots": {
      const r = p * 0.26;
      ctx.beginPath();
      let row = 0;
      for (let y = from; y < R + p; y += p * 0.5, row++) {
        const shift = row % 2 === 0 ? 0 : p / 2;
        for (let x = from + shift; x < R + p; x += p) {
          ctx.moveTo(x + r, y);
          ctx.arc(x, y, r, 0, Math.PI * 2);
        }
      }
      ctx.fill();
      break;
    }
    case "checks": {
      const c = p / 2;
      for (let y = from, j = 0; y < R; y += c, j++)
        for (let x = from, i = 0; x < R; x += c, i++) if ((i + j) % 2 === 0) ctx.fillRect(x, y, c, c);
      break;
    }
    case "gingham":
      // Two sets of bands in the same see-through colour; where they cross it is twice as strong.
      for (let x = from; x < R; x += p) ctx.fillRect(x, -R, p / 2, 2 * R);
      for (let y = from; y < R; y += p) ctx.fillRect(-R, y, 2 * R, p / 2);
      break;
    case "grid": {
      const t = Math.max(1.5, p * 0.06);
      for (let x = from; x < R; x += p) ctx.fillRect(x - t / 2, -R, t, 2 * R);
      for (let y = from; y < R; y += p) ctx.fillRect(-R, y - t / 2, 2 * R, t);
      break;
    }
    case "waves": {
      const amp = p * 0.22;
      ctx.lineWidth = p * 0.2;
      ctx.lineCap = "round";
      ctx.beginPath();
      for (let y = from; y < R + p; y += p) {
        for (let x = -R; x <= R; x += 6) {
          const yy = y + amp * Math.sin((x / p) * Math.PI);
          if (x === -R) ctx.moveTo(x, yy);
          else ctx.lineTo(x, yy);
        }
      }
      ctx.stroke();
      break;
    }
    case "zigzag": {
      const amp = p * 0.25;
      ctx.lineWidth = p * 0.18;
      ctx.lineJoin = "miter";
      ctx.beginPath();
      for (let y = from; y < R + p; y += p) {
        let up = true;
        for (let x = -R - p; x <= R + p; x += p / 2, up = !up) {
          const yy = y + (up ? -amp : amp);
          if (x === -R - p) ctx.moveTo(x, yy);
          else ctx.lineTo(x, yy);
        }
      }
      ctx.stroke();
      break;
    }
    case "halftone": {
      // Dots that grow from nothing at the top to touching at the bottom.
      ctx.beginPath();
      for (let y = from; y < R + p; y += p) {
        const t = Math.min(1, Math.max(0, (y + R) / (2 * R)));
        const r = (p / 2) * 1.05 * t;
        if (r < 0.3) continue;
        for (let x = from; x < R + p; x += p) {
          ctx.moveTo(x + r, y);
          ctx.arc(x, y, r, 0, Math.PI * 2);
        }
      }
      ctx.fill();
      break;
    }
    case "confetti": {
      const rand = seeded(layer.seed);
      const colors = confettiColors(layer.color).map(cssColor);
      const count = Math.round(((2 * R) / p) ** 2 * 6);
      for (let i = 0; i < count; i++) {
        const x = -R + rand() * 2 * R;
        const y = -R + rand() * 2 * R;
        const s = p * (0.07 + rand() * 0.09);
        ctx.fillStyle = colors[Math.floor(rand() * colors.length)];
        ctx.save();
        ctx.translate(x, y);
        ctx.rotate(rand() * Math.PI);
        if (rand() < 0.35) {
          ctx.beginPath();
          ctx.arc(0, 0, s * 0.55, 0, Math.PI * 2);
          ctx.fill();
        } else ctx.fillRect(-s, -s * 0.35, s * 2, s * 0.7);
        ctx.restore();
      }
      break;
    }
  }
  ctx.restore();
}

/** Pixels of a grain tile, `side` square: specks of `color` at random strengths. */
export function grainPixels(side: number, seed: number, color: string): Uint8ClampedArray {
  const c = parseColor(color) ?? { r: 255, g: 255, b: 255, a: 1 };
  const rand = seeded(seed);
  const data = new Uint8ClampedArray(side * side * 4);
  for (let i = 0; i < side * side; i++) {
    const v = rand();
    data[i * 4] = c.r;
    data[i * 4 + 1] = c.g;
    data[i * 4 + 2] = c.b;
    data[i * 4 + 3] = Math.round(255 * c.a * v * v);
  }
  return data;
}
