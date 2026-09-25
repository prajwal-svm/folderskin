/**
 * Shows a design on the folder. The folder's pixels are the template layers Rust rendered
 * (`composer_template`): coverage masks of the back and front panels, the paper and back rim
 * between them, and the front's rims on top. Stacking the design between them here gives the
 * same picture Rust makes when it saves the icon (the Rust tests check that the layers add up
 * to its render), and it's cheap enough to redo on every frame of a drag.
 */
import type { Shape } from "./doc";
// Not the composer's own catalog: the browser preview (lib/devMock.ts) draws with this too, and
// that catalog comes with the composer's chunk.
import { t } from "../i18n";

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
    img.onerror = () => reject(new Error(t("common.errors.templateLoad")));
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

/** How the editor shows what's see-through: a fine checkerboard, in greys that sit on any backdrop. */
const CHECK_CELLS = 40;
const CHECK_LIGHT = "rgba(120, 128, 140, 0.10)";
const CHECK_DARK = "rgba(120, 128, 140, 0.22)";
/** The folder's edges where the design leaves it see-through. */
const SKELETON_LINE = "rgba(110, 118, 130, 0.6)";

let checker: HTMLCanvasElement | null = null;
function checkerboard(px: number): HTMLCanvasElement {
  if (!checker) checker = document.createElement("canvas");
  if (checker.width !== px) {
    checker.width = px;
    checker.height = px;
    const g = checker.getContext("2d")!;
    const cell = px / CHECK_CELLS;
    g.fillStyle = CHECK_LIGHT;
    g.fillRect(0, 0, px, px);
    g.fillStyle = CHECK_DARK;
    for (let y = 0; y < CHECK_CELLS; y++) {
      for (let x = y % 2; x < CHECK_CELLS; x += 2) g.fillRect(Math.round(x * cell), Math.round(y * cell), Math.ceil(cell), Math.ceil(cell));
    }
  }
  return checker;
}

/**
 * The folder as it is with nothing on it, for the editor only: a checkerboard over its whole
 * shape (back panel with its tab, and front) and its edges as a fine grey line. A design drawn
 * over it hides both where it's solid, so an empty or see-through design still shows exactly the
 * folder it will be, never a bare sheet of paper with a rim floating under it.
 */
function drawSkeleton(ctx: Ctx, t: TemplateImages, px: number, scratch: HTMLCanvasElement) {
  if (scratch.width !== px || scratch.height !== px) {
    scratch.width = px;
    scratch.height = px;
  }
  const s = scratch.getContext("2d")!;
  reset(s);
  s.clearRect(0, 0, px, px);
  s.drawImage(t.back, 0, 0, px, px);
  s.drawImage(t.front, 0, 0, px, px);
  s.globalCompositeOperation = "source-in";
  s.drawImage(checkerboard(px), 0, 0);
  s.globalCompositeOperation = "source-over";
  ctx.save();
  reset(ctx);
  ctx.drawImage(scratch, 0, 0, px, px);
  ctx.restore();
  drawOutline(ctx, t, px, scratch, SKELETON_LINE, 1);
}

/** The design on the folder as the editor shows it: the folder's skeleton under whatever of it is see-through. */
export function drawFolderView(ctx: Ctx, design: CanvasImageSource, t: TemplateImages, px: number, scratch: HTMLCanvasElement) {
  drawSkeleton(ctx, t, px, scratch);
  drawOnFolder(ctx, design, t, px, scratch);
}

/** A free icon as the editor shows it: the checkerboard under whatever of the square is see-through. */
export function drawFreeView(ctx: Ctx, design: CanvasImageSource, px: number) {
  ctx.save();
  reset(ctx);
  ctx.drawImage(checkerboard(px), 0, 0, px, px);
  ctx.drawImage(design, 0, 0, px, px);
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
    if (view.skeleton) drawFolderView(ctx, design, t, px, scratch);
    else {
      ctx.save();
      reset(ctx);
      ctx.drawImage(design, 0, 0, px, px);
      ctx.restore();
      drawOutline(ctx, t, px, scratch, view.guide, 0.9);
    }
    return;
  }
  if (view.skeleton) {
    drawOnFolder(ctx, greyDesign(px), t, px, scratch, 0.32);
    ctx.save();
    reset(ctx);
    ctx.drawImage(design, 0, 0, px, px);
    ctx.restore();
    return;
  }
  drawFreeView(ctx, design, px);
}
