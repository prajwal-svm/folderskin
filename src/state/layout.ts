/**
 * The window's three columns: the sidebar (full, or folded to a rail of icons), the centre island,
 * and the island on the right. The sidebar and the right island can be dragged wider or narrower;
 * the centre always keeps room to work in. Pure, so every rule here is unit-tested.
 */

const KEY = "folderskin.layout";

/**
 * What the user chose: the sidebar's width and whether it's folded, the right island's width
 * (null: the default), and whether the folder panel on the right is closed.
 */
export type Layout = { left: number; rail: boolean; right: number | null; rightClosed: boolean };

export const LEFT = { min: 196, max: 320, initial: 228 } as const;
/** The folded sidebar: an island of icons. */
export const RAIL = 64;
/** Dragging the sidebar narrower than this folds it; dragging a folded one wider than this opens it. */
export const FOLD_AT = 150;
export const UNFOLD_AT = 120;
export const RIGHT = { min: 300, max: 480 } as const;
/** The least the centre island keeps, whatever the sides ask for. */
export const CENTRE_MIN = 440;
/** The gap between islands and around the window's edge (--gap in tokens.css). */
export const GAP = 10;

export const DEFAULT_LAYOUT: Layout = { left: LEFT.initial, rail: false, right: null, rightClosed: false };

const clamp = (v: number, min: number, max: number) => Math.min(Math.max(v, min), Math.max(min, max));

/** The right island's width before it's dragged: 29% of the window, between 320 and 372. */
export function defaultRight(windowWidth: number): number {
  return Math.round(clamp(windowWidth * 0.29, 320, 372));
}

/** Reads a saved layout, keeping only what makes sense. */
export function readLayout(raw: string | null): Layout {
  try {
    const v = JSON.parse(raw ?? "{}") as Partial<Record<keyof Layout, unknown>>;
    const left = typeof v.left === "number" && Number.isFinite(v.left) ? clamp(Math.round(v.left), LEFT.min, LEFT.max) : LEFT.initial;
    const right = typeof v.right === "number" && Number.isFinite(v.right) ? clamp(Math.round(v.right), RIGHT.min, RIGHT.max) : null;
    return { left, rail: v.rail === true, right, rightClosed: v.rightClosed === true };
  } catch {
    return DEFAULT_LAYOUT;
  }
}

export function loadLayout(): Layout {
  try {
    return readLayout(localStorage.getItem(KEY));
  } catch {
    return DEFAULT_LAYOUT;
  }
}

export function saveLayout(layout: Layout) {
  try {
    localStorage.setItem(KEY, JSON.stringify(layout));
  } catch {
    // Only a preference.
  }
}

/**
 * The widths the columns actually get in a window `windowWidth` wide, with or without the right
 * island. The sides give way, the right one first, so the centre keeps `CENTRE_MIN`; a sidebar
 * never gets narrower than its minimum (it folds to the rail by choice, not by squeezing).
 */
export function columns(layout: Layout, windowWidth: number, rightShown: boolean): { left: number; right: number } {
  // Around the islands: the gaps between them, and the window's edge on the right (and on the
  // left too when the sidebar is a rail, which floats like the others).
  const gaps = (rightShown ? 2 : 1) * GAP + GAP + (layout.rail ? GAP : 0);
  const left = layout.rail
    ? RAIL
    : clamp(Math.min(layout.left, windowWidth - gaps - (rightShown ? RIGHT.min : 0) - CENTRE_MIN), LEFT.min, LEFT.max);
  if (!rightShown) return { left, right: 0 };
  const wanted = layout.right ?? defaultRight(windowWidth);
  const room = windowWidth - gaps - left - CENTRE_MIN;
  return { left, right: clamp(Math.min(wanted, room), RIGHT.min, RIGHT.max) };
}

/** The sidebar dragged to `width`: its new width, folded to the rail below `FOLD_AT`. */
export function dragSidebar(layout: Layout, width: number): Layout {
  if (layout.rail) {
    return width > UNFOLD_AT ? { ...layout, rail: false, left: clamp(Math.round(width), LEFT.min, LEFT.max) } : layout;
  }
  if (width < FOLD_AT) return { ...layout, rail: true };
  return { ...layout, left: clamp(Math.round(width), LEFT.min, LEFT.max) };
}

/**
 * The sidebar's edge moved `by` with the arrow keys from `width`, the width it has now. A step
 * narrower from its narrowest folds it and a step wider from the rail opens it again, at the
 * width it had; a drag keeps its own thresholds, so a slight one from the rail doesn't open it.
 */
export function stepSidebar(layout: Layout, width: number, by: number): Layout {
  if (layout.rail) return by > 0 ? { ...layout, rail: false } : layout;
  if (by < 0 && width <= LEFT.min) return { ...layout, rail: true };
  return dragSidebar(layout, width + by);
}

/** The right island dragged to `width`, within its limits and the room the window has. */
export function dragRight(layout: Layout, width: number, windowWidth: number): Layout {
  const left = layout.rail ? RAIL : layout.left;
  const room = windowWidth - 3 * GAP - (layout.rail ? GAP : 0) - left - CENTRE_MIN;
  return { ...layout, right: clamp(Math.round(Math.min(width, room)), RIGHT.min, RIGHT.max) };
}
