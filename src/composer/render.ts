/**
 * Draws a design: its layers, bottom to top, onto a square canvas of any size. The same code
 * draws the stage, the thumbnails and the full-size picture that is sent to Rust to become the
 * icon, so what the user sees is what gets saved.
 *
 * This draws only the user's own art. The folder around it comes from the Rust template layers
 * (composite.ts), never from here.
 */
import type { Assets } from "./assets";
import { cssColor, embossLight, embossTint } from "./color";
import { backgroundColor, CANVAS, isPlaced, mainColor, type Doc, type FillLayer, type IconLayer, type Layer, type Paint, type PlacedLayer } from "./doc";
import { EMOJI_STACK } from "./fonts";
import { bounds, type Box, type Rect } from "./geometry";
import { drawPattern } from "./patterns";
import { evenOdd, roundRect, traceShape } from "./shapes";

type Ctx = CanvasRenderingContext2D;

/** What a layer needs to know about the design around it: the folder's colour, for icons pressed into it. */
export type Surround = { folder: string | null };
const NO_SURROUND: Surround = { folder: null };

/** How far a pressed-in icon's lip and shade reach, in canvas units. */
const embossReach = (layer: IconLayer) => (layer.look === "emboss" ? (layer.size * 0.011 * layer.depth) / 60 : 0);

/** The colour an icon is drawn in, before any pressing in. */
export function iconColor(layer: IconLayer, around: Surround): string {
  if (layer.look === "original" && layer.brand) return layer.brand;
  if (layer.look === "emboss" && layer.auto) return embossTint(around.folder);
  return mainColor(layer.paint);
}

/** A canvas style for a paint laid over the rectangle `x, y, w, h`. */
export function paintStyle(ctx: Ctx, paint: Paint, x: number, y: number, w: number, h: number): string | CanvasGradient {
  if (paint.type === "solid") return cssColor(paint.color);
  let g: CanvasGradient;
  if (paint.type === "linear") {
    // CSS angles: 0 points up, 90 to the right. The line is long enough to reach the corners.
    const t = (paint.angle * Math.PI) / 180;
    const dx = Math.sin(t);
    const dy = -Math.cos(t);
    const len = Math.abs(w * dx) + Math.abs(h * dy);
    const cx = x + w / 2;
    const cy = y + h / 2;
    g = ctx.createLinearGradient(cx - (dx * len) / 2, cy - (dy * len) / 2, cx + (dx * len) / 2, cy + (dy * len) / 2);
  } else {
    const cx = x + paint.cx * w;
    const cy = y + paint.cy * h;
    const r = Math.max(...[x, x + w].flatMap((px) => [y, y + h].map((py) => Math.hypot(px - cx, py - cy))), 1);
    g = ctx.createRadialGradient(cx, cy, 0, cx, cy, r);
  }
  for (const s of paint.stops) g.addColorStop(Math.min(1, Math.max(0, s.at)), cssColor(s.color));
  return g;
}

/** The box a placed layer takes on the canvas. */
export function boxOf(layer: PlacedLayer, assets: Assets): Box {
  switch (layer.kind) {
    case "text": {
      const l = assets.layout(layer);
      return { x: layer.x, y: layer.y, w: l.w, h: l.h, rotation: layer.rotation };
    }
    case "emoji":
    case "icon":
      return { x: layer.x, y: layer.y, w: layer.size, h: layer.size, rotation: layer.rotation };
    case "shape":
    case "image":
      return { x: layer.x, y: layer.y, w: layer.w, h: layer.h, rotation: layer.rotation };
  }
}

/** How far a layer's ink can reach past its box: its outline, edge and shadow. */
function reach(layer: PlacedLayer): number {
  let r = 2;
  if (layer.kind === "text") r += layer.size * 0.25 + (layer.stroke?.width ?? 0);
  if (layer.kind === "emoji") r += layer.size * 0.12;
  if (layer.kind === "shape") r += layer.stroke?.width ?? 0;
  if (layer.kind === "icon") r += (layer.strokeWidth * layer.size) / layer.viewBox / 2 + embossReach(layer);
  if (layer.edge) r += layer.edge.width;
  return r;
}

const shadowReach = (layer: Layer) =>
  isPlaced(layer) && layer.shadow ? layer.shadow.blur * 1.5 + Math.max(Math.abs(layer.shadow.x), Math.abs(layer.shadow.y)) : 0;

/** The canvas rectangle a layer's ink can cover, before its shadow. */
export function inkRect(layer: Layer, assets: Assets): Rect {
  if (!isPlaced(layer)) return { x0: 0, y0: 0, x1: CANVAS, y1: CANVAS };
  return bounds(boxOf(layer, assets), reach(layer));
}

function place(ctx: Ctx, layer: PlacedLayer) {
  ctx.translate(layer.x, layer.y);
  if (layer.rotation) ctx.rotate((layer.rotation * Math.PI) / 180);
  if (layer.flipX || layer.flipY) ctx.scale(layer.flipX ? -1 : 1, layer.flipY ? -1 : 1);
}

/** Draws an icon's paths, centred on the origin, `size` canvas units square. */
function drawIcon(ctx: Ctx, layer: IconLayer, assets: Assets, color: string) {
  const s = layer.size / layer.viewBox;
  ctx.translate(-layer.size / 2, -layer.size / 2);
  ctx.scale(s, s);
  ctx.fillStyle = color;
  ctx.strokeStyle = color;
  ctx.lineWidth = layer.strokeWidth;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  const rule = layer.evenOdd ? "evenodd" : "nonzero";
  layer.paths.forEach((d, i) => {
    const p = assets.path(d);
    if (layer.style === "fill" || layer.filled.includes(i)) ctx.fill(p, rule);
    else ctx.stroke(p);
  });
}

/** Draws one layer's own pixels, without its opacity, blend, edge or shadow. */
export function drawContent(ctx: Ctx, layer: Layer, assets: Assets, around: Surround = NO_SURROUND) {
  ctx.save();
  switch (layer.kind) {
    case "fill":
      ctx.fillStyle = paintStyle(ctx, layer.paint, 0, 0, CANVAS, CANVAS);
      ctx.fillRect(0, 0, CANVAS, CANVAS);
      break;
    case "pattern":
      drawPattern(ctx, layer, (seed, color) => assets.grain(seed, color));
      break;
    case "text": {
      place(ctx, layer);
      const l = assets.layout(layer);
      ctx.font = l.font;
      ctx.textBaseline = "middle";
      const fill = paintStyle(ctx, layer.paint, -l.w / 2, -l.h / 2, l.w, l.h);
      const each = (draw: (s: string, x: number, y: number) => void) => {
        if (l.glyphs) {
          ctx.textAlign = "center";
          for (const g of l.glyphs) {
            if (!g.rot) draw(g.ch, g.x, g.y);
            else {
              ctx.save();
              ctx.translate(g.x, g.y);
              ctx.rotate(g.rot);
              draw(g.ch, 0, 0);
              ctx.restore();
            }
          }
        } else {
          ctx.textAlign = "left";
          for (const line of l.lines) draw(line.text, line.x, line.y);
        }
      };
      if (layer.stroke && layer.stroke.width > 0) {
        // Twice as wide, with the fill drawn over its inner half: the outline sits outside the letters.
        ctx.lineWidth = layer.stroke.width * 2;
        ctx.lineJoin = "round";
        ctx.miterLimit = 2;
        ctx.strokeStyle = cssColor(layer.stroke.color);
        each((s, x, y) => ctx.strokeText(s, x, y));
      }
      ctx.fillStyle = fill;
      each((s, x, y) => ctx.fillText(s, x, y));
      break;
    }
    case "emoji": {
      place(ctx, layer);
      ctx.font = `${layer.size}px ${EMOJI_STACK}`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      const m = ctx.measureText(layer.char);
      const asc = m.actualBoundingBoxAscent;
      const desc = m.actualBoundingBoxDescent;
      const dy = Number.isFinite(asc) && Number.isFinite(desc) && asc + desc > 0 ? (asc - desc) / 2 : 0;
      ctx.fillStyle = "#000";
      ctx.fillText(layer.char, 0, dy);
      break;
    }
    case "shape": {
      place(ctx, layer);
      ctx.beginPath();
      traceShape(ctx, layer);
      ctx.fillStyle = paintStyle(ctx, layer.paint, -layer.w / 2, -layer.h / 2, layer.w, layer.h);
      ctx.fill(evenOdd(layer) ? "evenodd" : "nonzero");
      if (layer.stroke && layer.stroke.width > 0) {
        ctx.lineWidth = layer.stroke.width;
        ctx.lineJoin = "round";
        ctx.strokeStyle = cssColor(layer.stroke.color);
        ctx.stroke();
      }
      break;
    }
    case "image": {
      place(ctx, layer);
      const pic = assets.picture(layer.src, layer.fx);
      if (!pic) break;
      const { w, h, iw, ih } = layer;
      if (layer.radius > 0) {
        ctx.beginPath();
        roundRect(ctx, -w / 2, -h / 2, w, h, (layer.radius * Math.min(w, h)) / 2);
        ctx.clip();
      }
      // The picture covers its box, cropped around its middle.
      const s = Math.max(w / iw, h / ih);
      const sw = w / s;
      const sh = h / s;
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(pic, (iw - sw) / 2, (ih - sh) / 2, sw, sh, -w / 2, -h / 2, w, h);
      break;
    }
    case "icon":
      place(ctx, layer);
      drawIcon(ctx, layer, assets, iconColor(layer, around));
      break;
  }
  ctx.restore();
}

/**
 * Presses a layer drawn alone on `src` into the folder: a lighter copy a little lower shows as
 * the groove's lit lower lip, and the layer less itself moved down leaves its top edges, shaded.
 * Drawn with compositing, not `ctx.filter`, which the macOS 12 web view doesn't have.
 */
function pressIn(src: HTMLCanvasElement, dst: HTMLCanvasElement, band: HTMLCanvasElement, d: number, light: { lip: string; shade: string }): HTMLCanvasElement {
  const w = src.width;
  const h = src.height;
  const g = dst.getContext("2d")!;
  g.setTransform(1, 0, 0, 1, 0, 0);
  g.globalCompositeOperation = "source-over";
  g.clearRect(0, 0, w, h);
  g.drawImage(src, 0, d);
  g.globalCompositeOperation = "source-in";
  g.fillStyle = light.lip;
  g.fillRect(0, 0, w, h);
  g.globalCompositeOperation = "source-over";
  g.drawImage(src, 0, 0);

  const b = band.getContext("2d")!;
  b.setTransform(1, 0, 0, 1, 0, 0);
  b.globalCompositeOperation = "source-over";
  b.clearRect(0, 0, w, h);
  b.drawImage(src, 0, 0);
  b.globalCompositeOperation = "destination-out";
  b.drawImage(src, 0, d * 0.75);
  b.globalCompositeOperation = "source-in";
  b.fillStyle = light.shade;
  b.fillRect(0, 0, w, h);
  b.globalCompositeOperation = "source-over";
  g.drawImage(band, 0, 0);
  return dst;
}

/** Grows a layer drawn alone on `src` by `r` pixels in `color` behind it: a sticker's edge. */
function withEdge(src: HTMLCanvasElement, dst: HTMLCanvasElement, r: number, color: string): HTMLCanvasElement {
  const d = dst.getContext("2d")!;
  d.setTransform(1, 0, 0, 1, 0, 0);
  d.globalCompositeOperation = "source-over";
  d.clearRect(0, 0, dst.width, dst.height);
  const rings = r > 6 ? [r, r * 0.66, r * 0.33] : [r];
  for (const ring of rings) {
    const steps = Math.max(8, Math.min(48, Math.ceil(ring * 1.4)));
    for (let i = 0; i < steps; i++) {
      const a = (i / steps) * Math.PI * 2;
      d.drawImage(src, Math.cos(a) * ring, Math.sin(a) * ring);
    }
  }
  d.globalCompositeOperation = "source-in";
  d.fillStyle = cssColor(color);
  d.fillRect(0, 0, dst.width, dst.height);
  d.globalCompositeOperation = "source-over";
  d.drawImage(src, 0, 0);
  return dst;
}

/** Whether a layer has to be drawn on its own first: anything that applies to it as a whole. */
function isolated(layer: Layer): boolean {
  return (
    layer.opacity < 1 ||
    layer.blend !== "normal" ||
    (layer.kind === "icon" && embossReach(layer) > 0) ||
    (isPlaced(layer) && (layer.shadow !== null || (layer.edge !== null && layer.edge.width > 0)))
  );
}

export function drawLayer(ctx: Ctx, layer: Layer, k: number, px: number, assets: Assets, around: Surround = NO_SURROUND) {
  if (!isolated(layer)) {
    drawContent(ctx, layer, assets, around);
    return;
  }
  const r = inkRect(layer, assets);
  // Nothing of it, not even its shadow, can reach the canvas.
  const s = shadowReach(layer);
  if (r.x1 + s < 0 || r.y1 + s < 0 || r.x0 - s > CANVAS || r.y0 - s > CANVAS) return;
  // The layer's whole ink area, so its edge and shadow see all of it (a huge one is cut down).
  const ox0 = Math.max(Math.floor(r.x0 * k), -px);
  const oy0 = Math.max(Math.floor(r.y0 * k), -px);
  const ow = Math.min(Math.ceil(r.x1 * k), 2 * px) - ox0;
  const oh = Math.min(Math.ceil(r.y1 * k), 2 * px) - oy0;
  if (ow <= 0 || oh <= 0) return;
  const off = assets.scratch(0, ow, oh);
  const o = off.getContext("2d")!;
  o.setTransform(1, 0, 0, 1, 0, 0);
  o.globalAlpha = 1;
  o.globalCompositeOperation = "source-over";
  o.clearRect(0, 0, off.width, off.height);
  o.setTransform(k, 0, 0, k, -ox0, -oy0);
  drawContent(o, layer, assets, around);
  let img: HTMLCanvasElement = off;
  if (layer.kind === "icon" && embossReach(layer) > 0) {
    img = pressIn(img, assets.scratch(2, ow, oh), assets.scratch(3, ow, oh), Math.max(1, embossReach(layer) * k), embossLight(around.folder));
  }
  if (isPlaced(layer) && layer.edge && layer.edge.width > 0) img = withEdge(img, assets.scratch(img === off ? 1 : 0, ow, oh), layer.edge.width * k, layer.edge.color);

  ctx.save();
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.globalAlpha = layer.opacity;
  ctx.globalCompositeOperation = layer.blend === "normal" ? "source-over" : layer.blend;
  if (isPlaced(layer) && layer.shadow) {
    ctx.shadowColor = cssColor(layer.shadow.color);
    ctx.shadowBlur = layer.shadow.blur * k;
    ctx.shadowOffsetX = layer.shadow.x * k;
    ctx.shadowOffsetY = layer.shadow.y * k;
  }
  ctx.drawImage(img, 0, 0, ow, oh, ox0, oy0, ow, oh);
  ctx.restore();
}

/**
 * A fill that covers only the folder's front panel: drawn on a spare canvas, cut to the front's
 * mask (the one Rust cuts the design's front with, so the two meet exactly), then laid on with
 * the layer's opacity and blend.
 */
function drawOnFront(ctx: Ctx, layer: FillLayer, k: number, px: number, assets: Assets, mask: CanvasImageSource) {
  const off = assets.scratch(0, px, px);
  const o = off.getContext("2d")!;
  o.setTransform(1, 0, 0, 1, 0, 0);
  o.globalAlpha = 1;
  o.globalCompositeOperation = "source-over";
  o.clearRect(0, 0, px, px);
  o.setTransform(k, 0, 0, k, 0, 0);
  drawContent(o, layer, assets);
  o.setTransform(1, 0, 0, 1, 0, 0);
  o.globalCompositeOperation = "destination-in";
  o.drawImage(mask, 0, 0, px, px);
  o.globalCompositeOperation = "source-over";
  ctx.save();
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.globalAlpha = layer.opacity;
  ctx.globalCompositeOperation = layer.blend === "normal" ? "source-over" : layer.blend;
  ctx.drawImage(off, 0, 0, px, px, 0, 0, px, px);
  ctx.restore();
}

/** Draws `doc` onto `ctx`, a canvas `px` pixels square, from scratch; `only` draws one layer alone. */
export function renderDoc(ctx: Ctx, doc: Doc, px: number, assets: Assets, opts: { only?: string } = {}) {
  const k = px / CANVAS;
  ctx.save();
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.globalAlpha = 1;
  ctx.globalCompositeOperation = "source-over";
  ctx.clearRect(0, 0, px, px);
  ctx.setTransform(k, 0, 0, k, 0, 0);
  const around: Surround = { folder: backgroundColor(doc) };
  for (const layer of doc.layers) {
    if (layer.hidden) continue;
    if (opts.only && layer.id !== opts.only) continue;
    // Until the folder's template has loaded there's no front to cut to: it covers everything.
    const front = layer.kind === "fill" && layer.part === "front" && doc.shape === "folder" ? assets.front(doc.style) : null;
    if (front && layer.kind === "fill") drawOnFront(ctx, layer, k, px, assets, front);
    else drawLayer(ctx, layer, k, px, assets, around);
    ctx.setTransform(k, 0, 0, k, 0, 0);
  }
  ctx.restore();
}
