import { useEffect, useId, useState, type CSSProperties, type KeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode, type Ref } from "react";
import { ChevronDownIcon } from "../icons/composer";

const PANELS_KEY = "folderskin.composer.panels";
/** How tall the layers are while the settings are open too, until the bar between them is dragged. */
export const LAYERS_HEIGHT = 232;
/** The least each keeps while both are open: the layers a heading and a row, the settings a few controls. */
export const MIN_LAYERS = 84;
export const MIN_SETTINGS = 140;
/** How far an arrow key moves the bar. */
const STEP = 16;

/** Which of the side's two panels are open, and how tall the layers are when both are. */
export type Panels = { layers: boolean; settings: boolean; height: number };

function loadPanels(): Panels {
  try {
    const v = JSON.parse(localStorage.getItem(PANELS_KEY) ?? "{}") as Partial<Record<keyof Panels, unknown>>;
    const height = typeof v.height === "number" && Number.isFinite(v.height) ? Math.max(MIN_LAYERS, Math.round(v.height)) : LAYERS_HEIGHT;
    return { layers: v.layers !== false, settings: v.settings !== false, height };
  } catch {
    return { layers: true, settings: true, height: LAYERS_HEIGHT };
  }
}

/** The side's panels as they were left on this computer; saved a moment after each change. */
export function usePanels() {
  const [panels, setPanels] = useState(loadPanels);
  useEffect(() => {
    const t = window.setTimeout(() => {
      try {
        localStorage.setItem(PANELS_KEY, JSON.stringify(panels));
      } catch {
        // Only a preference.
      }
    }, 250);
    return () => window.clearTimeout(t);
  }, [panels]);
  return [panels, setPanels] as const;
}

/**
 * One of the side's panels: a heading that opens and closes it, with room for a count and for
 * buttons of its own, and what's in it.
 */
export function Panel({
  title,
  badge,
  actions,
  open,
  onToggle,
  className,
  style,
  panelRef,
  children,
}: {
  title: string;
  /** Beside the title, such as how many layers there are. */
  badge?: ReactNode;
  /** Buttons at the heading's end, shown open or closed. */
  actions?: ReactNode;
  open: boolean;
  onToggle: () => void;
  className?: string;
  style?: CSSProperties;
  panelRef?: Ref<HTMLElement>;
  children: ReactNode;
}) {
  const body = useId();
  return (
    <section ref={panelRef} className={`cmp-panel${open ? " is-open" : ""}${className ? ` ${className}` : ""}`} style={style}>
      <div className="cmp-panel-head">
        <button type="button" className="cmp-panel-toggle" aria-expanded={open} aria-controls={body} onClick={onToggle}>
          <span className="cmp-panel-chevron" aria-hidden="true">
            <ChevronDownIcon size={14} />
          </span>
          <span className="cmp-panel-title" title={title}>
            {title}
          </span>
          {badge}
        </button>
        {actions && <div className="cmp-panel-actions">{actions}</div>}
      </div>
      {open && (
        <div id={body} className="cmp-panel-body">
          {children}
        </div>
      )}
    </section>
  );
}

const clamp = (v: number, min: number, max: number) => Math.min(max, Math.max(min, v));

/**
 * The bar between the two panels: drag it, or focus it and use the arrow keys, to give the layers
 * more room or less. A double click puts it back where it started. `measure` says where it is now
 * and how far it can go, read from the page as the drag starts.
 */
export function Resizer({
  measure,
  onHeight,
  onReset,
}: {
  measure: () => { height: number; min: number; max: number };
  onHeight: (height: number) => void;
  onReset: () => void;
}) {
  const [dragging, setDragging] = useState(false);
  const [now, setNow] = useState<{ height: number; min: number; max: number } | null>(null);

  const drag = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const el = e.currentTarget;
    el.setPointerCapture(e.pointerId);
    const startY = e.clientY;
    const { height, min, max } = measure();
    setDragging(true);
    // The whole window keeps the resize cursor while the bar is held, wherever the pointer goes.
    document.documentElement.classList.add("is-resizing");
    const move = (ev: PointerEvent) => onHeight(Math.round(clamp(height + ev.clientY - startY, min, max)));
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      document.documentElement.classList.remove("is-resizing");
      setDragging(false);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
  };

  const keys = (e: KeyboardEvent<HTMLDivElement>) => {
    const m = measure();
    const next =
      e.key === "ArrowUp" ? m.height - STEP : e.key === "ArrowDown" ? m.height + STEP : e.key === "Home" ? m.min : e.key === "End" ? m.max : null;
    if (next === null) return;
    // The canvas nudges the selected layer with the arrows; these are the bar's.
    e.preventDefault();
    e.stopPropagation();
    const height = clamp(next, m.min, m.max);
    onHeight(height);
    setNow({ ...m, height });
  };

  return (
    <div
      role="separator"
      aria-orientation="horizontal"
      aria-label="Resize the layers"
      aria-valuenow={now?.height}
      aria-valuemin={now?.min}
      aria-valuemax={now?.max}
      tabIndex={0}
      className={dragging ? "cmp-resizer is-dragging" : "cmp-resizer"}
      title="Drag to give the layers more room. Double-click to reset."
      onPointerDown={drag}
      onKeyDown={keys}
      onFocus={() => setNow(measure())}
      onDoubleClick={onReset}
    >
      <span className="cmp-resizer-grip" aria-hidden="true" />
    </div>
  );
}
