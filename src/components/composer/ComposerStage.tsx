import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
import { AWAY_MS, makeCanvas, ctx2d, type Assets } from "../../composer/assets";
import { drawView, type TemplateImages, type View } from "../../composer/composite";
import { CANVAS, centreOf, findLayer, isCovering, isPlaced, patchLayer, type Doc, type Layer, type Parts, type PlacedLayer } from "../../composer/doc";
import {
  ALL_SIDES,
  boxTargets,
  contains,
  CORNERS,
  cursorFor,
  handlePoint,
  hitHandle,
  resizeBox,
  rotateBy,
  snap,
  type Box,
  type Guide,
  type Handle,
  type Point,
  type Targets,
} from "../../composer/geometry";
import { boxOf, renderDoc } from "../../composer/render";
import { RotateCwIcon } from "../icons/composer";

/** Room around the canvas for handles that reach past its edge, in screen pixels. */
const PAD = 30;
/** How far above a box its turn handle floats, and how near a handle the pointer must be. */
const ROT_GAP_PX = 24;
const HANDLE_PX = 11;
const SNAP_PX = 6;

export type Backdrop = "window" | "light" | "dark" | "colour";

const handlesFor = (l: PlacedLayer): Handle[] => (l.kind === "text" || l.kind === "emoji" ? [...CORNERS, "rot"] : [...ALL_SIDES, "rot"]);

type Drag =
  | { kind: "move"; id: string; doc0: Doc; start: Point; orig: Point; box0: Box; targets: Targets; moved: boolean; sx: number; sy: number }
  | { kind: "resize"; id: string; doc0: Doc; handle: Exclude<Handle, "rot">; box0: Box; layer0: PlacedLayer }
  | { kind: "rotate"; id: string; doc0: Doc; start: Point; box0: Box };

/** The layer's new fields for a resized box. Text and emoji keep their shape and change size. */
function resized(layer: PlacedLayer, box0: Box, box: Box): Record<string, number> {
  const place = { x: box.x, y: box.y };
  if (layer.kind === "text") return { ...place, size: Math.max(4, (layer.size * box.h) / box0.h) };
  if (layer.kind === "emoji") return { ...place, size: Math.max(4, box.w) };
  return { ...place, w: box.w, h: box.h };
}

/**
 * The composer's canvas. It draws the design (render.ts) and shows it on the folder or flat
 * (composite.ts), and turns the pointer into changes: pick a layer, drag it (snapping to the
 * folder's middle, its tab and other layers), size it from its corners and sides, turn it from
 * the knob above it. Selection is a soft tint over the layer, never an outline.
 */
export function ComposerStage({
  doc,
  selectedId,
  onSelect,
  onPreview,
  onSettle,
  assets,
  template,
  parts,
  view,
  backdrop,
  version,
  active,
  onOpen,
  hint,
}: {
  doc: Doc;
  selectedId: string | null;
  onSelect: (id: string | null) => void;
  /** A change while the pointer is still down. */
  onPreview: (doc: Doc) => void;
  /** The pointer let go: the change so far is one step. */
  onSettle: () => void;
  assets: Assets;
  template: TemplateImages | null;
  /** False while the composer is open but not on screen: nothing is drawn, and the scratch
   *  canvases are given back. */
  active: boolean;
  parts: Parts;
  view: View;
  backdrop: Backdrop;
  /** Goes up when a picture or font finishes loading, so the canvas draws again. */
  version: number;
  /** A layer was double-clicked: edit its words, pick another emoji. */
  onOpen: (layer: Layer) => void;
  /** Words shown over an empty design. */
  hint: string | null;
}) {
  const wrap = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const design = useRef<HTMLCanvasElement | null>(null);
  const scratch = useRef<HTMLCanvasElement | null>(null);
  const [area, setArea] = useState({ w: 0, h: 0 });
  const drag = useRef<Drag | null>(null);
  const [guides, setGuides] = useState<Guide[]>([]);
  const [hover, setHover] = useState<string | null>(null);
  const [cursor, setCursor] = useState("default");
  const [readout, setReadout] = useState<{ text: string; x: number; y: number } | null>(null);

  useLayoutEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const measure = () => setArea({ w: el.clientWidth, h: el.clientHeight });
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const size = Math.max(120, Math.floor(Math.min(area.w, area.h) - 2 * PAD));
  const left = Math.round((area.w - size) / 2);
  const top = Math.round((area.h - size) / 2);
  const scale = size / CANVAS;
  const dpr = typeof window === "undefined" ? 1 : Math.min(2.5, window.devicePixelRatio || 1);
  const px = Math.min(2048, Math.round(size * dpr));

  // Draws on the next frame, once, however many changes came in before it.
  const frame = useRef(0);
  const drawn = useRef(false);
  const latest = useRef({ doc, view, template, px });
  latest.current = { doc, view, template, px };
  useEffect(() => {
    cancelAnimationFrame(frame.current);
    if (!active) return;
    frame.current = requestAnimationFrame(() => {
      const c = canvas.current;
      if (!c) return;
      const { doc: d, view: v, template: t, px: n } = latest.current;
      // Given back while the composer was away and not back yet: the canvas still holds the last
      // frame, which is a better thing to show than the design without its folder.
      if (!t && drawn.current) return;
      if (c.width !== n) {
        c.width = n;
        c.height = n;
      }
      design.current ??= makeCanvas(n, n);
      scratch.current ??= makeCanvas(n, n);
      if (design.current.width !== n) {
        design.current.width = n;
        design.current.height = n;
      }
      renderDoc(ctx2d(design.current), d, n, assets);
      drawView(ctx2d(c), design.current, t, n, v, scratch.current);
      drawn.current = true;
    });
    return () => cancelAnimationFrame(frame.current);
  }, [doc, view, template, px, version, assets, active]);

  // Two canvases the size of the stage, only ever drawn on between frames. Away from the composer
  // they are the largest thing it holds, and the next draw makes them again.
  useEffect(() => {
    if (active) return;
    const t = window.setTimeout(() => {
      for (const ref of [design, scratch]) {
        if (!ref.current) continue;
        ref.current.width = 0;
        ref.current.height = 0;
        ref.current = null;
      }
    }, AWAY_MS);
    return () => window.clearTimeout(t);
  }, [active]);

  const toUnits = useCallback(
    (e: { clientX: number; clientY: number }): Point => {
      const r = wrap.current!.getBoundingClientRect();
      return { x: ((e.clientX - r.left - left) / size) * CANVAS, y: ((e.clientY - r.top - top) / size) * CANVAS };
    },
    [left, top, size],
  );

  const placed = useMemo(() => doc.layers.filter((l): l is PlacedLayer => isPlaced(l) && !l.hidden), [doc]);

  /** The topmost layer under a point that can be picked on the canvas. */
  const layerAt = useCallback(
    (p: Point): PlacedLayer | null => {
      for (let i = placed.length - 1; i >= 0; i--) {
        const l = placed[i];
        if (!l.locked && contains(boxOf(l, assets), p, 6 / scale)) return l;
      }
      return null;
    },
    [placed, assets, scale],
  );

  const selected = findLayer(doc, selectedId);
  const selBox = selected && isPlaced(selected) && !selected.hidden ? boxOf(selected, assets) : null;
  const gap = ROT_GAP_PX / scale;
  const radius = HANDLE_PX / scale;

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    const p = toUnits(e);
    const el = e.currentTarget;
    const begin = (d: Drag) => {
      drag.current = d;
      el.setPointerCapture(e.pointerId);
    };
    if (selected && isPlaced(selected) && selBox && !selected.locked) {
      const h = hitHandle(selBox, p, handlesFor(selected), radius, gap);
      if (h === "rot") return begin({ kind: "rotate", id: selected.id, doc0: doc, start: p, box0: selBox });
      if (h) return begin({ kind: "resize", id: selected.id, doc0: doc, handle: h, box0: selBox, layer0: selected });
    }
    const hit = layerAt(p);
    if (hit) {
      onSelect(hit.id);
      const [fx0, fy0, fx1, fy1] = parts.folder;
      const front = centreOf(parts.front);
      const tab = centreOf(parts.tab);
      const others = boxTargets(placed.filter((l) => l.id !== hit.id).map((l) => boxOf(l, assets)));
      const targets: Targets = {
        xs: [CANVAS / 2, tab.x, fx0, fx1, ...others.xs],
        ys: [front.y, CANVAS / 2, tab.y, parts.front[1], fy0, fy1, ...others.ys],
      };
      return begin({ kind: "move", id: hit.id, doc0: doc, start: p, orig: { x: hit.x, y: hit.y }, box0: boxOf(hit, assets), targets, moved: false, sx: e.clientX, sy: e.clientY });
    }
    const inside = p.x >= 0 && p.y >= 0 && p.x <= CANVAS && p.y <= CANVAS;
    const covering = inside ? [...doc.layers].reverse().find((l) => isCovering(l) && !l.hidden && !l.locked) : undefined;
    onSelect(covering?.id ?? null);
  };

  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    const p = toUnits(e);
    if (!d) {
      if (selected && isPlaced(selected) && selBox && !selected.locked) {
        const h = hitHandle(selBox, p, handlesFor(selected), radius, gap);
        if (h) {
          setCursor(cursorFor(h, selBox.rotation));
          setHover(null);
          return;
        }
      }
      const over = layerAt(p);
      setHover(over && over.id !== selectedId ? over.id : null);
      setCursor(over ? "move" : "default");
      return;
    }
    const r = wrap.current!.getBoundingClientRect();
    const at = { x: e.clientX - r.left + 14, y: e.clientY - r.top + 14 };
    if (d.kind === "move") {
      if (!d.moved && Math.hypot(e.clientX - d.sx, e.clientY - d.sy) < 3) return;
      d.moved = true;
      let x = d.orig.x + (p.x - d.start.x);
      let y = d.orig.y + (p.y - d.start.y);
      if (!(e.metaKey || e.ctrlKey)) {
        const s = snap({ ...d.box0, x, y }, d.targets, SNAP_PX / scale);
        x += s.dx;
        y += s.dy;
        setGuides(s.guides);
      } else setGuides([]);
      onPreview(patchLayer(d.doc0, d.id, { x, y }));
      return;
    }
    if (d.kind === "resize") {
      const l = d.layer0;
      const corner = d.handle.length === 2;
      const keepAspect =
        l.kind === "text" || l.kind === "emoji" ? true : l.kind === "image" ? corner !== e.shiftKey : corner && e.shiftKey;
      const box = resizeBox(d.box0, d.handle, p, { keepAspect, fromCenter: e.altKey });
      onPreview(patchLayer(d.doc0, d.id, resized(l, d.box0, box)));
      setReadout({ text: `${Math.round(box.w)} × ${Math.round(box.h)}`, ...at });
      return;
    }
    const rotation = rotateBy(d.box0, d.start, p, e.shiftKey);
    onPreview(patchLayer(d.doc0, d.id, { rotation }));
    setReadout({ text: `${Math.round(rotation)}°`, ...at });
    setCursor("grabbing");
  };

  const end = () => {
    const d = drag.current;
    drag.current = null;
    setGuides([]);
    setReadout(null);
    if (d && (d.kind !== "move" || d.moved)) onSettle();
  };

  const boxStyle = (b: Box): CSSProperties => ({
    left: left + b.x * scale,
    top: top + b.y * scale,
    width: Math.max(2, b.w * scale),
    height: Math.max(2, b.h * scale),
    transform: `translate(-50%, -50%) rotate(${b.rotation}deg)`,
  });
  const hovered = hover ? placed.find((l) => l.id === hover) : undefined;

  return (
    <div className="cmp-stage" data-backdrop={backdrop} ref={wrap}>
      <canvas className="cmp-canvas" ref={canvas} style={{ left, top, width: size, height: size }} aria-hidden="true" />
      {hint && (
        <p className="cmp-stage-hint" style={{ left, top: top + size / 2, width: size }}>
          {hint}
        </p>
      )}
      <div
        className="cmp-overlay"
        style={{ cursor }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={end}
        onPointerCancel={end}
        onPointerLeave={() => {
          if (!drag.current) setHover(null);
        }}
        onDoubleClick={(e) => {
          const hit = layerAt(toUnits(e));
          if (hit) onOpen(hit);
        }}
      >
        {hovered && <div className="cmp-hover" style={boxStyle(boxOf(hovered, assets))} />}
        {selBox && selected && isPlaced(selected) && (
          <>
            <div className={selected.locked ? "cmp-sel is-locked" : "cmp-sel"} style={boxStyle(selBox)} />
            {!selected.locked &&
              handlesFor(selected).map((h) => {
                const q = handlePoint(selBox, h, gap);
                return (
                  <span
                    key={h}
                    className={h === "rot" ? "cmp-knob is-rot" : "cmp-knob"}
                    style={{ left: left + q.x * scale, top: top + q.y * scale }}
                    aria-hidden="true"
                  >
                    {h === "rot" && <RotateCwIcon size={11} />}
                  </span>
                );
              })}
          </>
        )}
        {guides.map((g) =>
          g.axis === "x" ? (
            <span key={`x${g.at}`} className="cmp-guide is-x" style={{ left: left + g.at * scale, top, height: size }} />
          ) : (
            <span key={`y${g.at}`} className="cmp-guide is-y" style={{ top: top + g.at * scale, left, width: size }} />
          ),
        )}
        {readout && (
          <span className="cmp-readout" style={{ left: readout.x, top: readout.y }}>
            {readout.text}
          </span>
        )}
      </div>
    </div>
  );
}
