import { useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
import { cssColor } from "../../composer/color";
import { layerLabel, mainColor, type Doc, type Layer } from "../../composer/doc";
import { EyeOffIcon, EyeOpenIcon, LockIcon, LockOpenIcon, ShapesIcon, TrashIcon, TypeIcon, WavesIcon } from "../icons/composer";
import { paintCss } from "../../composer/paints";

const ROW = 38;

/** A layer's little picture in the list: its colour, its emoji, its picture, or its kind. */
function Thumb({ layer }: { layer: Layer }) {
  switch (layer.kind) {
    case "fill":
      return <span className="cmp-thumb is-paint" style={{ "--g": paintCss(layer.paint) } as CSSProperties} />;
    case "pattern":
      return (
        <span className="cmp-thumb is-kind" style={{ "--c": cssColor(layer.color) } as CSSProperties}>
          <WavesIcon size={14} />
        </span>
      );
    case "text":
      return (
        <span className="cmp-thumb is-kind" style={{ "--c": cssColor(mainColor(layer.paint)) } as CSSProperties}>
          <TypeIcon size={14} />
        </span>
      );
    case "emoji":
      return <span className="cmp-thumb is-emoji">{layer.char}</span>;
    case "shape":
      return (
        <span className="cmp-thumb is-kind" style={{ "--c": cssColor(mainColor(layer.paint)) } as CSSProperties}>
          <ShapesIcon size={14} />
        </span>
      );
    case "image":
      return <img className="cmp-thumb is-image" src={layer.src} alt="" draggable={false} />;
  }
}

/**
 * The layers, top first, as the eye sees them stacked. Click to select, double-click a name to
 * rename it, drag a row to restack it; each row can be hidden, locked or deleted.
 */
export function ComposerLayers({
  doc,
  selectedId,
  onSelect,
  onToggle,
  onRename,
  onMove,
  onDelete,
}: {
  doc: Doc;
  selectedId: string | null;
  onSelect: (id: string) => void;
  onToggle: (id: string, field: "hidden" | "locked") => void;
  onRename: (id: string, name: string) => void;
  /** Moves a layer to `index` in the document (0 is the bottom). */
  onMove: (id: string, index: number) => void;
  onDelete: (id: string) => void;
}) {
  const [renaming, setRenaming] = useState<string | null>(null);
  const [dragging, setDragging] = useState<{ id: string; slot: number; offset: number } | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const rows = [...doc.layers].reverse();
  const n = rows.length;

  const startDrag = (e: ReactPointerEvent<HTMLDivElement>, id: string, row: number) => {
    if (e.button !== 0 || renaming) return;
    const startY = e.clientY;
    const el = e.currentTarget;
    let active = false;
    let slot = row;
    const move = (ev: PointerEvent) => {
      const dy = ev.clientY - startY;
      if (!active && Math.abs(dy) < 4) return;
      if (!active) {
        active = true;
        el.setPointerCapture(ev.pointerId);
      }
      slot = Math.max(0, Math.min(n - 1, Math.round(row + dy / ROW)));
      setDragging({ id, slot, offset: dy });
    };
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      if (!active) return;
      setDragging(null);
      if (slot !== row) onMove(id, n - 1 - slot);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
  };

  if (n === 0) return <p className="cmp-layers-empty">Nothing here yet. Add a colour, words or a picture from the bar above the folder.</p>;

  return (
    <div className="cmp-layers" ref={list} role="listbox" aria-label="layers" style={{ height: n * ROW }}>
      {rows.map((layer, row) => {
        const index = n - 1 - row;
        const lifted = dragging?.id === layer.id;
        // Rows between where the dragged one was and where it would land slide out of its way.
        let shift = 0;
        if (dragging && !lifted) {
          const from = rows.findIndex((l) => l.id === dragging.id);
          if (from < row && dragging.slot >= row) shift = -ROW;
          if (from > row && dragging.slot <= row) shift = ROW;
        }
        const y = row * ROW + (lifted ? dragging.offset : shift);
        const label = layerLabel(layer, index);
        return (
          <div
            key={layer.id}
            role="option"
            aria-selected={layer.id === selectedId}
            className={`cmp-layer${layer.id === selectedId ? " is-on" : ""}${layer.hidden ? " is-hidden" : ""}${lifted ? " is-lifted" : ""}`}
            style={{ transform: `translateY(${y}px)` }}
            tabIndex={layer.id === selectedId || (!selectedId && row === 0) ? 0 : -1}
            onKeyDown={(e) => {
              if (renaming || e.target !== e.currentTarget) return;
              const step = e.key === "ArrowUp" ? -1 : e.key === "ArrowDown" ? 1 : 0;
              if (step) {
                e.preventDefault();
                const next = rows[row + step];
                if (!next) return;
                // ⌥ with an arrow restacks the layer instead of moving to the next one.
                if (e.altKey) onMove(layer.id, index - step);
                else onSelect(next.id);
                window.requestAnimationFrame(() => list.current?.querySelector<HTMLElement>(".cmp-layer.is-on")?.focus());
              } else if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onSelect(layer.id);
              } else if (e.key === "F2") {
                e.preventDefault();
                setRenaming(layer.id);
              }
            }}
            onPointerDown={(e) => {
              if ((e.target as Element).closest("button, input")) return;
              onSelect(layer.id);
              startDrag(e, layer.id, row);
            }}
            onDoubleClick={() => setRenaming(layer.id)}
          >
            <Thumb layer={layer} />
            {renaming === layer.id ? (
              <input
                className="cmp-layer-name-input"
                defaultValue={layer.name ?? label}
                autoFocus
                maxLength={40}
                aria-label="layer name"
                onFocus={(e) => e.currentTarget.select()}
                onBlur={(e) => {
                  onRename(layer.id, e.currentTarget.value.trim());
                  setRenaming(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") e.currentTarget.blur();
                  if (e.key === "Escape") {
                    e.stopPropagation();
                    setRenaming(null);
                  }
                }}
              />
            ) : (
              <span className="cmp-layer-name" title={label}>
                {label}
              </span>
            )}
            <button
              type="button"
              className={layer.locked ? "cmp-layer-btn is-set" : "cmp-layer-btn"}
              aria-label={layer.locked ? `unlock ${label}` : `lock ${label}`}
              title={layer.locked ? "Unlock" : "Lock, so it can't be moved on the canvas"}
              onClick={() => onToggle(layer.id, "locked")}
            >
              {layer.locked ? <LockIcon size={13} /> : <LockOpenIcon size={13} />}
            </button>
            <button
              type="button"
              className={layer.hidden ? "cmp-layer-btn is-set" : "cmp-layer-btn"}
              aria-label={layer.hidden ? `show ${label}` : `hide ${label}`}
              title={layer.hidden ? "Show" : "Hide"}
              onClick={() => onToggle(layer.id, "hidden")}
            >
              {layer.hidden ? <EyeOffIcon size={14} /> : <EyeOpenIcon size={14} />}
            </button>
            <button type="button" className="cmp-layer-btn is-danger" aria-label={`delete ${label}`} title="Delete" onClick={() => onDelete(layer.id)}>
              <TrashIcon size={13} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
