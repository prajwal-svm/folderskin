/**
 * Laying out a text layer: its lines, where each letter goes when the text is spaced out or
 * curved, and the box it takes. Measuring is passed in (the canvas does it in the app, a fake
 * one in the tests), so the layout itself is pure.
 */
import type { TextLayer } from "./doc";
import { fontStack } from "./fonts";

/** Width of `text` set in the CSS `font`, in the same units as the font's size. */
export type Measure = (font: string, text: string) => number;

/** One letter: its centre (on the line's middle) and turn in radians, in the layer's own frame. */
export type Glyph = { ch: string; x: number; y: number; rot: number };

export type TextLayout = {
  /** The CSS font the canvas draws with. */
  font: string;
  /** Straight text drawn a line at a time: each line's left edge and middle. */
  lines: { text: string; x: number; y: number; width: number }[];
  /** Spaced or curved text is drawn a letter at a time instead. */
  glyphs: Glyph[] | null;
  /** The box the text takes, centred on the layer's position. */
  w: number;
  h: number;
};

/** The CSS font string of a text layer. */
export function fontOf(layer: Pick<TextLayer, "italic" | "weight" | "size" | "font">): string {
  return `${layer.italic ? "italic " : ""}${layer.weight} ${layer.size}px ${fontStack(layer.font)}`;
}

type Segmenter = { segment: (s: string) => Iterable<{ segment: string }> };
const segmenter: Segmenter | null = (() => {
  const Seg = (Intl as unknown as { Segmenter?: new (l?: string, o?: { granularity: string }) => Segmenter }).Segmenter;
  return Seg ? new Seg(undefined, { granularity: "grapheme" }) : null;
})();

/** The letters of a line as people see them, so an emoji made of several code points stays whole. */
export function graphemes(s: string): string[] {
  return segmenter ? Array.from(segmenter.segment(s), (x) => x.segment) : Array.from(s);
}

/** The text as it is drawn: in capitals when the layer says so, one entry per line. */
export function linesOf(layer: Pick<TextLayer, "text" | "upper">): string[] {
  const t = layer.upper ? layer.text.toLocaleUpperCase() : layer.text;
  return t.split("\n");
}

export function layoutText(layer: TextLayer, measure: Measure): TextLayout {
  const font = fontOf(layer);
  const lines = linesOf(layer);
  const size = layer.size;
  const lh = size * layer.lineHeight;
  const gap = layer.spacing * size;
  const perLetter = gap !== 0 || layer.curve !== 0;

  // Each line's letters, where each starts (kerning kept by measuring the text before it) and its width.
  const measured = lines.map((line) => {
    const letters = graphemes(line);
    if (!perLetter) return { line, letters, starts: [] as number[], widths: [] as number[], width: measure(font, line) };
    const starts: number[] = [];
    const widths: number[] = [];
    let prefix = "";
    let before = 0;
    letters.forEach((ch, i) => {
      prefix += ch;
      const after = measure(font, prefix);
      starts.push(before + i * gap);
      widths.push(after - before);
      before = after;
    });
    const width = letters.length === 0 ? 0 : before + (letters.length - 1) * gap;
    return { line, letters, starts, widths, width };
  });

  const widest = Math.max(0, ...measured.map((m) => m.width));
  const n = lines.length;
  const middle = (i: number) => (i - (n - 1) / 2) * lh;

  if (!perLetter) {
    const w = widest;
    const out = measured.map((m, i) => {
      const x = layer.align === "left" ? -w / 2 : layer.align === "right" ? w / 2 - m.width : -m.width / 2;
      return { text: m.line, x, y: middle(i), width: m.width };
    });
    return { font, lines: out, glyphs: null, w: Math.max(w, size * 0.3), h: Math.max(n * lh, size * 0.5) };
  }

  const glyphs: Glyph[] = [];
  // Curved: every line on an arc of the same radius, the widest spanning the whole bend.
  const bend = (Math.abs(layer.curve) / 100) * Math.PI * 0.95;
  const radius = bend > 0 && widest > 0 ? widest / bend : Infinity;
  const arch = layer.curve > 0 ? 1 : -1;
  measured.forEach((m, i) => {
    const y0 = middle(i);
    const offset =
      layer.curve !== 0 ? -m.width / 2 : layer.align === "left" ? -widest / 2 : layer.align === "right" ? widest / 2 - m.width : -m.width / 2;
    m.letters.forEach((ch, k) => {
      const centre = offset + m.starts[k] + m.widths[k] / 2;
      if (radius === Infinity) {
        glyphs.push({ ch, x: centre, y: y0, rot: 0 });
        return;
      }
      const phi = centre / radius;
      glyphs.push({ ch, x: radius * Math.sin(phi), y: y0 + arch * radius * (1 - Math.cos(phi)), rot: arch * phi });
    });
  });

  // The box around every letter (each taken as its width by the size, turned), then centred.
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -Infinity;
  let y1 = -Infinity;
  let gi = 0;
  measured.forEach((m) => {
    m.widths.forEach((cw) => {
      const g = glyphs[gi++];
      const ex = Math.abs((cw / 2) * Math.cos(g.rot)) + Math.abs((size / 2) * Math.sin(g.rot));
      const ey = Math.abs((cw / 2) * Math.sin(g.rot)) + Math.abs((size / 2) * Math.cos(g.rot));
      x0 = Math.min(x0, g.x - ex);
      x1 = Math.max(x1, g.x + ex);
      y0 = Math.min(y0, g.y - ey);
      y1 = Math.max(y1, g.y + ey);
    });
  });
  if (!Number.isFinite(x0)) return { font, lines: [], glyphs: [], w: size * 0.3, h: Math.max(n * lh, size * 0.5) };
  const cx = (x0 + x1) / 2;
  const cy = (y0 + y1) / 2;
  for (const g of glyphs) {
    g.x -= cx;
    g.y -= cy;
  }
  return { font, lines: [], glyphs, w: Math.max(x1 - x0, size * 0.3), h: Math.max(y1 - y0, size * 0.5) };
}
