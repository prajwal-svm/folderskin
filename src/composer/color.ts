/**
 * Colours as the composer keeps them: `#rrggbb`, or `#rrggbbaa` when not fully opaque. Pure
 * helpers for the picker (HSV), for drawing (a CSS string every canvas accepts) and for picking
 * readable defaults (luminance).
 */

/** Red, green and blue from 0 to 255, alpha from 0 to 1. */
export type RGBA = { r: number; g: number; b: number; a: number };

/** Hue in degrees, saturation and value from 0 to 1. */
export type HSV = { h: number; s: number; v: number };

const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));
const byte = (n: number) => Math.round(clamp(n, 0, 255));

/** Reads `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`/`rgba()` and `transparent`. */
export function parseColor(input: string): RGBA | null {
  const s = input.trim().toLowerCase();
  if (s === "transparent") return { r: 0, g: 0, b: 0, a: 0 };
  const hex = /^#?([0-9a-f]{3,8})$/.exec(s);
  if (hex) {
    let h = hex[1];
    if (h.length === 3 || h.length === 4) h = [...h].map((c) => c + c).join("");
    if (h.length !== 6 && h.length !== 8) return null;
    const n = (i: number) => parseInt(h.slice(i, i + 2), 16);
    return { r: n(0), g: n(2), b: n(4), a: h.length === 8 ? Math.round((n(6) / 255) * 1000) / 1000 : 1 };
  }
  const fn = /^rgba?\(([^)]*)\)$/.exec(s);
  if (fn) {
    const parts = fn[1].split(/[\s,/]+/).filter(Boolean);
    if (parts.length < 3) return null;
    const channel = (p: string) => (p.endsWith("%") ? (parseFloat(p) / 100) * 255 : parseFloat(p));
    const alpha = parts[3] === undefined ? 1 : parts[3].endsWith("%") ? parseFloat(parts[3]) / 100 : parseFloat(parts[3]);
    const [r, g, b] = parts.slice(0, 3).map(channel);
    if ([r, g, b, alpha].some((n) => Number.isNaN(n))) return null;
    return { r: byte(r), g: byte(g), b: byte(b), a: clamp(alpha, 0, 1) };
  }
  return null;
}

const hex2 = (n: number) => byte(n).toString(16).padStart(2, "0");

/** `#rrggbb` when opaque, `#rrggbbaa` otherwise. */
export function toHex(c: RGBA): string {
  const base = `#${hex2(c.r)}${hex2(c.g)}${hex2(c.b)}`;
  return c.a >= 1 ? base : `${base}${hex2(c.a * 255)}`;
}

/** The colour as `rgba()`, which every canvas and stylesheet reads. Unknown input is transparent. */
export function cssColor(color: string): string {
  const c = parseColor(color);
  if (!c) return "rgba(0,0,0,0)";
  return `rgba(${c.r},${c.g},${c.b},${Math.round(c.a * 1000) / 1000})`;
}

/** A colour in its canonical form, or `fallback` when it can't be read. */
export function normalizeColor(color: unknown, fallback: string): string {
  if (typeof color !== "string") return fallback;
  const c = parseColor(color);
  return c ? toHex(c) : fallback;
}

export function alphaOf(color: string): number {
  return parseColor(color)?.a ?? 0;
}

/** The same colour with another alpha. */
export function withAlpha(color: string, a: number): string {
  const c = parseColor(color) ?? { r: 0, g: 0, b: 0, a: 1 };
  return toHex({ ...c, a: clamp(a, 0, 1) });
}

export function rgbToHsv({ r, g, b }: RGBA): HSV {
  const [R, G, B] = [r / 255, g / 255, b / 255];
  const max = Math.max(R, G, B);
  const min = Math.min(R, G, B);
  const d = max - min;
  let h = 0;
  if (d > 0) {
    if (max === R) h = ((G - B) / d) % 6;
    else if (max === G) h = (B - R) / d + 2;
    else h = (R - G) / d + 4;
    h *= 60;
    if (h < 0) h += 360;
  }
  return { h, s: max === 0 ? 0 : d / max, v: max };
}

export function hsvToRgb({ h, s, v }: HSV, a = 1): RGBA {
  const hh = (((h % 360) + 360) % 360) / 60;
  const c = v * s;
  const x = c * (1 - Math.abs((hh % 2) - 1));
  const m = v - c;
  const [r, g, b] =
    hh < 1 ? [c, x, 0] : hh < 2 ? [x, c, 0] : hh < 3 ? [0, c, x] : hh < 4 ? [0, x, c] : hh < 5 ? [x, 0, c] : [c, 0, x];
  return { r: byte((r + m) * 255), g: byte((g + m) * 255), b: byte((b + m) * 255), a };
}

/** Relative luminance (WCAG), 0 for black to 1 for white, ignoring alpha. */
export function luminance(color: string): number {
  const c = parseColor(color) ?? { r: 0, g: 0, b: 0, a: 1 };
  const lin = (v: number) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
}

/** White or near-black, whichever reads better on `background`. */
export function inkOn(background: string): string {
  const c = parseColor(background);
  if (!c || c.a < 0.35) return "#ffffff";
  return luminance(background) > 0.42 ? "#1b1f27" : "#ffffff";
}

/** A blend of two colours, `t` of the way from `a` to `b`. */
export function mix(a: string, b: string, t: number): string {
  const x = parseColor(a) ?? { r: 0, g: 0, b: 0, a: 1 };
  const y = parseColor(b) ?? { r: 0, g: 0, b: 0, a: 1 };
  const k = clamp(t, 0, 1);
  return toHex({ r: x.r + (y.r - x.r) * k, g: x.g + (y.g - x.g) * k, b: x.b + (y.b - x.b) * k, a: x.a + (y.a - x.a) * k });
}

/**
 * The colour of an icon pressed into a folder of colour `folder`, as macOS draws the symbol on
 * its folders: a deeper shade of the folder's own colour, or on a dark folder a lighter one, so
 * the symbol always reads while still belonging to the folder. With no folder colour (a
 * see-through design) it's the shade macOS uses on its own blue folder.
 */
export function embossTint(folder: string | null): string {
  const base = folder && (parseColor(folder)?.a ?? 0) > 0.35 ? toHex({ ...(parseColor(folder) as RGBA), a: 1 }) : FOLDER_BLUE_BOTTOM;
  const l = luminance(base);
  if (l < 0.06) return mix(base, "#ffffff", 0.26);
  // Lighter folders need a deeper shade to show; mid ones a little less.
  return mix(base, "#0c1a2c", l > 0.6 ? 0.3 : l > 0.25 ? 0.24 : 0.2);
}

/** The pressed-in look's lit lip and shaded edge for a folder whose colour is `folder`. */
export function embossLight(folder: string | null): { lip: string; shade: string } {
  const l = luminance(folder && parseColor(folder) ? folder : FOLDER_BLUE_BOTTOM);
  return { lip: l < 0.06 ? "#ffffff24" : l < 0.25 ? "#ffffff4d" : "#ffffff99", shade: l < 0.06 ? "#00000059" : "#0000003d" };
}

/** The picker's swatches: a spread of hues in a light, a clear and a deep version, then greys. */
export const SWATCHES = [
  "#ff5a5f", "#ff8a3d", "#ffc233", "#34c77b", "#1fb5a8", "#3a86ff", "#6f5cff", "#c150e8", "#ff5fa2",
  "#ffd1d1", "#ffe0c2", "#fff1b8", "#cdf3dd", "#c5efe9", "#d3e3ff", "#e1dcff", "#f1d7fb", "#ffd6e8",
  "#a4161a", "#b4501a", "#a77b00", "#136f43", "#0d6b64", "#1c4fb8", "#3a2aa8", "#6f1f8a", "#a31d5a",
  "#ffffff", "#e6e6e6", "#bdbdbd", "#8e8e8e", "#5e5e5e", "#3a3a3a", "#1f1f1f", "#000000",
];

/** The colour macOS gives a plain folder, for the default background. */
export const FOLDER_BLUE_TOP = "#7cc8f5";
export const FOLDER_BLUE_BOTTOM = "#4ea9e4";
