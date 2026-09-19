/**
 * Picture adjustments done on the pixels, once per change, rather than with canvas filters,
 * which the macOS 12 web view doesn't have. The colour adjustments fold into one affine matrix,
 * so a picture takes a single pass whatever is set; blur is three box blurs, close to a Gaussian.
 */
import type { ImageFx } from "./doc";

/** Rows of `[r, g, b, 1]` → channel, as 3 × 4 numbers. */
export type Matrix = [number, number, number, number, number, number, number, number, number, number, number, number];

const IDENTITY: Matrix = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0];

/** `a` after `b`: first `b`, then `a`. */
export function compose(a: Matrix, b: Matrix): Matrix {
  const out = new Array(12).fill(0) as Matrix;
  for (let r = 0; r < 3; r++) {
    for (let c = 0; c < 4; c++) {
      let v = 0;
      for (let k = 0; k < 3; k++) v += a[r * 4 + k] * b[k * 4 + c];
      if (c === 3) v += a[r * 4 + 3];
      out[r * 4 + c] = v;
    }
  }
  return out;
}

const LR = 0.2126;
const LG = 0.7152;
const LB = 0.0722;

const scale = (s: number): Matrix => [s, 0, 0, 0, 0, s, 0, 0, 0, 0, s, 0];
const saturate = (s: number): Matrix => [
  LR + (1 - LR) * s, LG - LG * s, LB - LB * s, 0,
  LR - LR * s, LG + (1 - LG) * s, LB - LB * s, 0,
  LR - LR * s, LG - LG * s, LB + (1 - LB) * s, 0,
];
/** The CSS `hue-rotate` matrix. */
function hueRotate(deg: number): Matrix {
  const a = (deg * Math.PI) / 180;
  const c = Math.cos(a);
  const s = Math.sin(a);
  return [
    0.213 + c * 0.787 - s * 0.213, 0.715 - c * 0.715 - s * 0.715, 0.072 - c * 0.072 + s * 0.928, 0,
    0.213 - c * 0.213 + s * 0.143, 0.715 + c * 0.285 + s * 0.14, 0.072 - c * 0.072 - s * 0.283, 0,
    0.213 - c * 0.213 - s * 0.787, 0.715 - c * 0.715 + s * 0.715, 0.072 + c * 0.928 + s * 0.072, 0,
  ];
}
const mixWith = (m: Matrix, t: number): Matrix => IDENTITY.map((v, i) => v + (m[i] - v) * t) as Matrix;
const SEPIA: Matrix = [0.393, 0.769, 0.189, 0, 0.349, 0.686, 0.168, 0, 0.272, 0.534, 0.131, 0];

/** The colour adjustments of `fx` as one matrix, in the order a photo editor applies them. */
export function fxMatrix(fx: ImageFx): Matrix {
  let m = IDENTITY;
  if (fx.brightness) m = compose(scale(1 + fx.brightness / 100), m);
  if (fx.contrast) {
    const k = Math.max(0, 1 + fx.contrast / 100);
    m = compose([k, 0, 0, 128 * (1 - k), 0, k, 0, 128 * (1 - k), 0, 0, k, 128 * (1 - k)], m);
  }
  if (fx.saturation) m = compose(saturate(Math.max(0, 1 + fx.saturation / 100)), m);
  if (fx.hue) m = compose(hueRotate(fx.hue), m);
  if (fx.grayscale) m = compose(mixWith(saturate(0), fx.grayscale / 100), m);
  if (fx.sepia) m = compose(mixWith(SEPIA, fx.sepia / 100), m);
  if (fx.invert) {
    const t = fx.invert / 100;
    m = compose([1 - 2 * t, 0, 0, 255 * t, 0, 1 - 2 * t, 0, 255 * t, 0, 0, 1 - 2 * t, 255 * t], m);
  }
  return m;
}

export const hasColorFx = (fx: ImageFx) =>
  Boolean(fx.brightness || fx.contrast || fx.saturation || fx.hue || fx.grayscale || fx.sepia || fx.invert);
export const hasFx = (fx: ImageFx) => hasColorFx(fx) || fx.blur > 0;

/** Applies a colour matrix to straight-alpha RGBA pixels, in place. */
export function applyMatrix(data: Uint8ClampedArray, m: Matrix) {
  for (let i = 0; i < data.length; i += 4) {
    const r = data[i];
    const g = data[i + 1];
    const b = data[i + 2];
    data[i] = m[0] * r + m[1] * g + m[2] * b + m[3];
    data[i + 1] = m[4] * r + m[5] * g + m[6] * b + m[7];
    data[i + 2] = m[8] * r + m[9] * g + m[10] * b + m[11];
  }
}

/** One box blur pass along rows (`horizontal`) or columns, `src` to `dst`, radius `r` pixels. */
function boxPass(src: Float32Array, dst: Float32Array, w: number, h: number, r: number, horizontal: boolean) {
  const len = horizontal ? w : h;
  const lines = horizontal ? h : w;
  const step = horizontal ? 4 : w * 4;
  const span = 2 * r + 1;
  for (let line = 0; line < lines; line++) {
    const base = horizontal ? line * w * 4 : line * 4;
    for (let c = 0; c < 4; c++) {
      let sum = 0;
      // The window starts centred on the first pixel, edges clamped.
      for (let k = -r; k <= r; k++) sum += src[base + Math.min(len - 1, Math.max(0, k)) * step + c];
      for (let i = 0; i < len; i++) {
        dst[base + i * step + c] = sum / span;
        const out = Math.max(0, i - r);
        const inn = Math.min(len - 1, i + r + 1);
        sum += src[base + inn * step + c] - src[base + out * step + c];
      }
    }
  }
}

/**
 * Blurs straight-alpha RGBA pixels in place by about `radius` pixels (a Gaussian's sigma of
 * roughly radius / 2). Works on premultiplied values, so transparent edges don't turn dark.
 */
export function blurPixels(data: Uint8ClampedArray, w: number, h: number, radius: number) {
  const r = Math.max(0, Math.round(radius / 2));
  if (r < 1 || w < 2 || h < 2) return;
  const a = new Float32Array(data.length);
  for (let i = 0; i < data.length; i += 4) {
    const al = data[i + 3] / 255;
    a[i] = data[i] * al;
    a[i + 1] = data[i + 1] * al;
    a[i + 2] = data[i + 2] * al;
    a[i + 3] = data[i + 3];
  }
  const b = new Float32Array(data.length);
  for (let pass = 0; pass < 3; pass++) {
    boxPass(a, b, w, h, r, true);
    boxPass(b, a, w, h, r, false);
  }
  for (let i = 0; i < data.length; i += 4) {
    const al = a[i + 3];
    data[i + 3] = al;
    if (al > 0) {
      const k = 255 / al;
      data[i] = a[i] * k;
      data[i + 1] = a[i + 1] * k;
      data[i + 2] = a[i + 2] * k;
    }
  }
}

/** Blur radius in the picture's own pixels: `fx.blur` is in canvas units for a picture 1024 wide. */
export const blurRadius = (fx: ImageFx, iw: number, ih: number) => (fx.blur * Math.max(iw, ih)) / 1024;
