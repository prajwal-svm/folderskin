import { useRef, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";

/** How far an arrow key moves the edge, and with ⇧. */
const STEP = 16;
const BIG_STEP = 48;

/**
 * The edge between two islands, in the gap between them: drag it (or focus it and use the arrow
 * keys) to make the island beside it wider or narrower, double-click it to put the width back.
 * It shows only while pointed at or held, as a thin line, so the islands keep their clean gap.
 *
 * `grows` says which way dragging widens the island: to the right for the sidebar, to the left for
 * the island on the right.
 */
export function IslandResizer({
  label,
  className,
  width,
  min,
  max,
  grows,
  onWidth,
  onStep,
  onReset,
}: {
  label: string;
  className: string;
  /** The island's width now. */
  width: number;
  min: number;
  max: number;
  grows: "right" | "left";
  /** Asks for a width; the layout decides what it becomes (the sidebar folds when narrow). */
  onWidth: (width: number) => void;
  /** Asks for the width to change by `by` with the arrow keys, for an island with rules of its own for a step; `onWidth` otherwise. */
  onStep?: (by: number) => void;
  onReset: () => void;
}) {
  const sign = grows === "right" ? 1 : -1;
  const start = useRef<{ x: number; width: number } | null>(null);

  const down = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const el = e.currentTarget;
    el.setPointerCapture(e.pointerId);
    start.current = { x: e.clientX, width };
    // The whole window keeps the resize cursor, and no text is selected, while the edge is held.
    document.documentElement.classList.add("is-resizing-islands");
    const move = (ev: PointerEvent) => {
      if (start.current) onWidth(start.current.width + sign * (ev.clientX - start.current.x));
    };
    const up = () => {
      start.current = null;
      document.documentElement.classList.remove("is-resizing-islands");
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
  };

  const key = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? BIG_STEP : STEP;
    const by = { ArrowRight: step, ArrowLeft: -step }[e.key];
    if (by !== undefined) {
      e.preventDefault();
      if (onStep) onStep(sign * by);
      else onWidth(width + sign * by);
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      onWidth(e.key === "Home" ? min : max);
    } else if (e.key === "Enter") {
      e.preventDefault();
      onReset();
    }
  };

  return (
    <div
      className={`island-resizer ${className}`}
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={Math.round(width)}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onPointerDown={down}
      onDoubleClick={onReset}
      onKeyDown={key}
    />
  );
}
