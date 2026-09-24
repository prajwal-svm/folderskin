/** Where a tooltip sits beside its element. */
export type TipSide = "top" | "bottom" | "left" | "right";

type Rect = { left: number; top: number; right: number; bottom: number };
type Size = { w: number; h: number };

/** Space between a tip and its element, and the least it keeps from the window's edges. */
export const TIP_GAP = 8;
export const TIP_MARGIN = 6;

const OPPOSITE: Record<TipSide, TipSide> = { top: "bottom", bottom: "top", left: "right", right: "left" };

function fits(side: TipSide, r: Rect, tip: Size, win: Size): boolean {
  switch (side) {
    case "top":
      return r.top - TIP_GAP - tip.h >= TIP_MARGIN;
    case "bottom":
      return r.bottom + TIP_GAP + tip.h <= win.h - TIP_MARGIN;
    case "left":
      return r.left - TIP_GAP - tip.w >= TIP_MARGIN;
    case "right":
      return r.right + TIP_GAP + tip.w <= win.w - TIP_MARGIN;
  }
}

const clamp = (v: number, min: number, max: number) => Math.min(Math.max(v, min), Math.max(min, max));

/**
 * The top-left corner of a tooltip of `tip` size for an element at `r`: on the side it prefers,
 * or the opposite one when there's no room there, centred on the element and slid along to stay
 * inside the window.
 */
export function placeTip(r: Rect, tip: Size, win: Size, prefer: TipSide): { x: number; y: number; side: TipSide } {
  const side = fits(prefer, r, tip, win) || !fits(OPPOSITE[prefer], r, tip, win) ? prefer : OPPOSITE[prefer];
  const cx = (r.left + r.right) / 2;
  const cy = (r.top + r.bottom) / 2;
  let x: number;
  let y: number;
  if (side === "top" || side === "bottom") {
    x = cx - tip.w / 2;
    y = side === "top" ? r.top - TIP_GAP - tip.h : r.bottom + TIP_GAP;
  } else {
    x = side === "left" ? r.left - TIP_GAP - tip.w : r.right + TIP_GAP;
    y = cy - tip.h / 2;
  }
  return {
    x: Math.round(clamp(x, TIP_MARGIN, win.w - TIP_MARGIN - tip.w)),
    y: Math.round(clamp(y, TIP_MARGIN, win.h - TIP_MARGIN - tip.h)),
    side,
  };
}
