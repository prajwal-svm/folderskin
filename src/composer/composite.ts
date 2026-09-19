/**
 * Shows a design on the folder. The folder's pixels are the template layers Rust rendered
 * (`composer_template`): coverage masks of the back and front panels, the paper and back rim
 * between them, and the front's rims on top. Stacking the design between them here gives the
 * same picture Rust makes when it saves the icon (the Rust tests check that the layers add up
 * to its render), and it's cheap enough to redo on every frame of a drag.
 */
import type { Shape } from "./doc";

export type TemplateImages = {
  back: CanvasImageSource;
  front: CanvasImageSource;
  middle: CanvasImageSource;
  top: CanvasImageSource;
  outline: CanvasImageSource;
};

export type TemplateSources = { back: string; front: string; middle: string; top: string; outline: string };

function load(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("couldn't load the folder template"));
    img.src = src;
  });
}

export async function loadTemplate(src: TemplateSources): Promise<TemplateImages> {
  const [back, front, middle, top, outline] = await Promise.all([src.back, src.front, src.middle, src.top, src.outline].map(load));
  return { back, front, middle, top, outline };
}

type Ctx = CanvasRenderingContext2D;

function reset(c: Ctx) {
  c.setTransform(1, 0, 0, 1, 0, 0);
  c.globalAlpha = 1;
  c.globalCompositeOperation = "source-over";
  c.shadowColor = "transparent";
}

/** `source` cut to one of the template's masks, on `scratch`. */
function masked(scratch: HTMLCanvasElement, source: CanvasImageSource, mask: CanvasImageSource, px: number): HTMLCanvasElement {
  const s = scratch.getContext("2d")!;
  reset(s);
  s.clearRect(0, 0, px, px);
  s.drawImage(source, 0, 0, px, px);
  s.globalCompositeOperation = "destination-in";
  s.drawImage(mask, 0, 0, px, px);
  s.globalCompositeOperation = "source-over";
  return scratch;
}

/** The design on the folder: back panel, paper and rim, front panel, front rims. */
export function drawOnFolder(ctx: Ctx, design: CanvasImageSource, t: TemplateImages, px: number, scratch: HTMLCanvasElement, alpha = 1) {
  if (scratch.width !== px || scratch.height !== px) {
    scratch.width = px;
    scratch.height = px;
  }
  ctx.save();
  reset(ctx);
  ctx.globalAlpha = alpha;
  ctx.drawImage(masked(scratch, design, t.back, px), 0, 0, px, px);
  ctx.drawImage(t.middle, 0, 0, px, px);
  ctx.drawImage(masked(scratch, design, t.front, px), 0, 0, px, px);
  ctx.drawImage(t.top, 0, 0, px, px);
  ctx.restore();
}

/** The folder's edges as a line in `color`. */
function drawOutline(ctx: Ctx, t: TemplateImages, px: number, scratch: HTMLCanvasElement, color: string, alpha: number) {
  if (scratch.width !== px || scratch.height !== px) {
    scratch.width = px;
    scratch.height = px;
  }
  const s = scratch.getContext("2d")!;
  reset(s);
  s.clearRect(0, 0, px, px);
  s.drawImage(t.outline, 0, 0, px, px);
  s.globalCompositeOperation = "source-in";
  s.fillStyle = color;
  s.fillRect(0, 0, px, px);
  s.globalCompositeOperation = "source-over";
  ctx.save();
  reset(ctx);
  ctx.globalAlpha = alpha;
  ctx.drawImage(scratch, 0, 0, px, px);
  ctx.restore();
}

let grey: HTMLCanvasElement | null = null;
/** A flat grey the size of a design, for the folder shown behind a free icon. */
function greyDesign(px: number): HTMLCanvasElement {
  if (!grey) grey = document.createElement("canvas");
  if (grey.width !== px) {
    grey.width = px;
    grey.height = px;
    const g = grey.getContext("2d")!;
    g.fillStyle = "#c9ced6";
    g.fillRect(0, 0, px, px);
  }
  return grey;
}

export type View = {
  shape: Shape;
  /** The folder skeleton switch: on, the design is seen on the folder (or, for a free icon, beside a faint one). */
  skeleton: boolean;
  /** The colour the folder's edges are drawn in when the design is shown flat. */
  guide: string;
};

/**
 * The stage's picture of a design: on the folder with the skeleton on, flat with the folder's
 * edges drawn over it with the skeleton off. A free icon is the design itself, with a faint
 * folder behind it for scale while the skeleton is on.
 */
export function drawView(ctx: Ctx, design: HTMLCanvasElement, t: TemplateImages | null, px: number, view: View, scratch: HTMLCanvasElement) {
  ctx.save();
  reset(ctx);
  ctx.clearRect(0, 0, px, px);
  ctx.restore();
  if (!t) {
    ctx.save();
    reset(ctx);
    ctx.drawImage(design, 0, 0, px, px);
    ctx.restore();
    return;
  }
  if (view.shape === "folder") {
    if (view.skeleton) drawOnFolder(ctx, design, t, px, scratch);
    else {
      ctx.save();
      reset(ctx);
      ctx.drawImage(design, 0, 0, px, px);
      ctx.restore();
      drawOutline(ctx, t, px, scratch, view.guide, 0.9);
    }
    return;
  }
  if (view.skeleton) drawOnFolder(ctx, greyDesign(px), t, px, scratch, 0.32);
  ctx.save();
  reset(ctx);
  ctx.drawImage(design, 0, 0, px, px);
  ctx.restore();
}
