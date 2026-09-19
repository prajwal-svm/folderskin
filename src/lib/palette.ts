/**
 * What colour a skin is, read from its own pixels, for the gallery's colour filter: up to three
 * colour names that each cover a fair part of the folder, and whether it's light or dark overall.
 */

export type Colour =
  | "red"
  | "orange"
  | "yellow"
  | "green"
  | "teal"
  | "blue"
  | "purple"
  | "pink"
  | "brown"
  | "black"
  | "grey"
  | "white";

export type Tone = "light" | "dark";

export type Palette = { colours: Colour[]; tone: Tone };

/** The colours the filter offers, in the order it shows them, with the swatch it draws. */
export const COLOURS: { id: Colour; label: string; swatch: string }[] = [
  { id: "red", label: "Red", swatch: "#e5484d" },
  { id: "orange", label: "Orange", swatch: "#f76b15" },
  { id: "yellow", label: "Yellow", swatch: "#f5c518" },
  { id: "green", label: "Green", swatch: "#30a46c" },
  { id: "teal", label: "Teal", swatch: "#12a594" },
  { id: "blue", label: "Blue", swatch: "#3a86ff" },
  { id: "purple", label: "Purple", swatch: "#8e4ec6" },
  { id: "pink", label: "Pink", swatch: "#e93d82" },
  { id: "brown", label: "Brown", swatch: "#8d5a2b" },
  { id: "black", label: "Black", swatch: "#1d1d1f" },
  { id: "grey", label: "Grey", swatch: "#8b8d98" },
  { id: "white", label: "White", swatch: "#f4f4f5" },
];

/** A colour counts when at least this share of the folder is that colour. */
const MIN_SHARE = 0.12;
const MAX_COLOURS = 3;
/** Below this average brightness (0 to 1) a skin is dark. */
const DARK_BELOW = 0.45;

/** The colour name a person would give one pixel. */
export function colourOf(r: number, g: number, b: number): Colour {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const value = max / 255;
  const saturation = max === 0 ? 0 : (max - min) / max;
  if (value < 0.16) return "black";
  if (saturation < 0.16 || max - min < 28) {
    if (value > 0.86) return "white";
    if (value < 0.3) return "black";
    return "grey";
  }
  const hue = hueOf(r, g, b, max, min);
  const pastel = value > 0.88 && saturation < 0.5;
  // A pale red is what people call pink.
  if (hue < 12 || hue >= 345) return pastel ? "pink" : "red";
  // Browns are dark or muted oranges and yellows: wood, earth, old varnish. Pale ones (peach,
  // apricot) stay orange.
  if (hue < 42) return value < 0.62 || (saturation < 0.45 && !pastel) ? "brown" : "orange";
  if (hue < 68) return value < 0.5 ? "brown" : "yellow";
  if (hue < 160) return "green";
  if (hue < 195) return "teal";
  if (hue < 255) return "blue";
  if (hue < 290) return "purple";
  return "pink";
}

function hueOf(r: number, g: number, b: number, max: number, min: number): number {
  const d = max - min;
  let h: number;
  if (max === r) h = ((g - b) / d) % 6;
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  return (h * 60 + 360) % 360;
}

/**
 * The palette of an image from its RGBA pixels. Pixels that are mostly transparent (around the
 * folder's shape) don't count.
 */
export function paletteOf(rgba: ArrayLike<number>): Palette {
  const counts = new Map<Colour, number>();
  let seen = 0;
  let light = 0;
  for (let i = 0; i + 3 < rgba.length; i += 4) {
    if (rgba[i + 3] < 128) continue;
    const r = rgba[i];
    const g = rgba[i + 1];
    const b = rgba[i + 2];
    const colour = colourOf(r, g, b);
    counts.set(colour, (counts.get(colour) ?? 0) + 1);
    light += (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
    seen += 1;
  }
  if (seen === 0) return { colours: [], tone: "light" };
  const ranked = [...counts].sort((a, b) => b[1] - a[1]);
  const colours = ranked.filter(([, n], i) => i === 0 || n / seen >= MIN_SHARE).slice(0, MAX_COLOURS).map(([c]) => c);
  return { colours, tone: light / seen < DARK_BELOW ? "dark" : "light" };
}

/** Side of the square a picture is shrunk to before it's read: plenty for its main colours. */
const SAMPLE = 48;

/** Reads the palette of a picture (a data URL or an address the page can load). */
export async function readPalette(src: string): Promise<Palette> {
  const image = new Image();
  image.decoding = "async";
  image.src = src;
  await image.decode();
  const canvas = document.createElement("canvas");
  canvas.width = SAMPLE;
  canvas.height = SAMPLE;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) throw new Error("no 2D canvas");
  ctx.drawImage(image, 0, 0, SAMPLE, SAMPLE);
  return paletteOf(ctx.getImageData(0, 0, SAMPLE, SAMPLE).data);
}
