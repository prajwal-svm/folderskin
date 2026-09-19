/**
 * What drawing a design needs besides the document: its pictures decoded (and adjusted),
 * grain tiles, text measured and laid out, and spare canvases to draw a layer on by itself.
 * One `Assets` lives as long as the composer; it tells its listeners when a picture has
 * finished loading so the stage can draw again.
 */
import type { Doc, ImageFx, TextLayer } from "./doc";
import { applyMatrix, blurPixels, blurRadius, fxMatrix, hasColorFx, hasFx } from "./imagefx";
import { grainPixels } from "./patterns";
import { layoutText, type TextLayout } from "./text";

const fxKey = (fx: ImageFx) =>
  `${fx.brightness},${fx.contrast},${fx.saturation},${fx.hue},${fx.blur},${fx.grayscale},${fx.sepia},${fx.invert}`;

export function makeCanvas(w: number, h: number): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = Math.max(1, Math.round(w));
  c.height = Math.max(1, Math.round(h));
  return c;
}

export function ctx2d(c: HTMLCanvasElement): CanvasRenderingContext2D {
  const ctx = c.getContext("2d");
  if (!ctx) throw new Error("no 2D canvas");
  return ctx;
}

type Picture = { img: HTMLImageElement; ready: boolean; failed: boolean; waiters: (() => void)[] };

export class Assets {
  private pictures = new Map<string, Picture>();
  /** Adjusted copies of a picture: only the latest adjustment of each is kept. */
  private adjusted = new Map<string, { key: string; canvas: HTMLCanvasElement }>();
  private grains = new Map<string, HTMLCanvasElement>();
  private layouts = new Map<string, TextLayout>();
  private scratchPool: HTMLCanvasElement[] = [];
  private listeners = new Set<() => void>();
  private measurer: CanvasRenderingContext2D | null = null;

  onChange(fn: () => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  private changed() {
    for (const fn of this.listeners) fn();
  }

  /** The decoded picture, or null while it loads (a listener hears when it's ready). */
  image(src: string): HTMLImageElement | null {
    let p = this.pictures.get(src);
    if (!p) {
      const img = new Image();
      p = { img, ready: false, failed: false, waiters: [] };
      const entry = p;
      img.onload = () => {
        entry.ready = true;
        entry.waiters.splice(0).forEach((w) => w());
        this.changed();
      };
      img.onerror = () => {
        entry.failed = true;
        entry.waiters.splice(0).forEach((w) => w());
        this.changed();
      };
      img.src = src;
      this.pictures.set(src, p);
    }
    return p.ready ? p.img : null;
  }

  /** The picture with its adjustments applied, or the picture itself when there are none. */
  picture(src: string, fx: ImageFx): CanvasImageSource | null {
    const img = this.image(src);
    if (!img) return null;
    if (!hasFx(fx)) return img;
    const key = fxKey(fx);
    const hit = this.adjusted.get(src);
    if (hit && hit.key === key) return hit.canvas;
    const w = img.naturalWidth;
    const h = img.naturalHeight;
    const canvas = hit?.canvas ?? makeCanvas(w, h);
    canvas.width = w;
    canvas.height = h;
    const c = ctx2d(canvas);
    c.clearRect(0, 0, w, h);
    c.drawImage(img, 0, 0);
    const data = c.getImageData(0, 0, w, h);
    if (hasColorFx(fx)) applyMatrix(data.data, fxMatrix(fx));
    if (fx.blur > 0) blurPixels(data.data, w, h, blurRadius(fx, w, h));
    c.putImageData(data, 0, 0);
    this.adjusted.set(src, { key, canvas });
    return canvas;
  }

  /** A 128 px tile of grain in `color`. */
  grain(seed: number, color: string): HTMLCanvasElement {
    const key = `${seed}|${color}`;
    let tile = this.grains.get(key);
    if (!tile) {
      tile = makeCanvas(128, 128);
      const c = ctx2d(tile);
      const data = c.createImageData(128, 128);
      data.data.set(grainPixels(128, seed, color));
      c.putImageData(data, 0, 0);
      if (this.grains.size > 24) this.grains.clear();
      this.grains.set(key, tile);
    }
    return tile;
  }

  /** Measures text with a canvas of its own. */
  measure = (font: string, text: string): number => {
    if (!this.measurer) this.measurer = ctx2d(makeCanvas(4, 4));
    this.measurer.font = font;
    return this.measurer.measureText(text).width;
  };

  /** A text layer's layout, kept while its words and look stay the same. */
  layout(layer: TextLayer): TextLayout {
    const key = JSON.stringify([layer.text, layer.font, layer.weight, layer.italic, layer.size, layer.align, layer.lineHeight, layer.spacing, layer.upper, layer.curve]);
    let hit = this.layouts.get(key);
    if (!hit) {
      hit = layoutText(layer, this.measure);
      if (this.layouts.size > 300) this.layouts.clear();
      this.layouts.set(key, hit);
    }
    return hit;
  }

  /** Forgets text layouts, for when a font has finished loading and measures differently. */
  fontsChanged() {
    this.layouts.clear();
    this.changed();
  }

  /** Spare canvas number `slot`, at least `w` by `h` (it is cleared by resizing). */
  scratch(slot: number, w: number, h: number): HTMLCanvasElement {
    let c = this.scratchPool[slot];
    if (!c) {
      c = makeCanvas(w, h);
      this.scratchPool[slot] = c;
    }
    const W = Math.max(1, Math.ceil(w));
    const H = Math.max(1, Math.ceil(h));
    if (c.width !== W || c.height !== H) {
      c.width = W;
      c.height = H;
    }
    return c;
  }

  /** Resolves once every picture in `doc` has loaded (or failed), so a render for saving has them all. */
  ready(doc: Doc): Promise<void> {
    const waits = doc.layers
      .filter((l) => l.kind === "image" && !l.hidden)
      .map((l) => {
        const src = (l as { src: string }).src;
        this.image(src);
        const p = this.pictures.get(src);
        if (!p || p.ready || p.failed) return Promise.resolve();
        return new Promise<void>((resolve) => p.waiters.push(resolve));
      });
    return Promise.all(waits).then(() => undefined);
  }

  /** Drops pictures the document no longer uses. */
  prune(docs: Doc[]) {
    const used = new Set<string>();
    for (const d of docs) for (const l of d.layers) if (l.kind === "image") used.add(l.src);
    for (const src of [...this.pictures.keys()]) if (!used.has(src)) this.pictures.delete(src);
    for (const src of [...this.adjusted.keys()]) if (!used.has(src)) this.adjusted.delete(src);
  }
}
