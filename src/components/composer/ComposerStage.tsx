import "../../i18n/composer";
import { t } from "../../i18n";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
import { makeCanvas, ctx2d, type Assets } from "../../composer/assets";
import { baseSelection, frameMatrix, framedBox, invertMatrix, applyMatrix, partAt, partSelection, type Matrix } from "../../composer/base";
import { mix, withAlpha } from "../../composer/color";
import { drawView, makeScratch, needsUpper, type Onto, type Scratch, type TemplateImages, type View } from "../../composer/composite";
import {
  CANVAS,
  centreOf,
  coveringTop,
  findLayer,
  HOME_FRAME,
  isCovering,
  isHome,
  isPlaced,
  MAX_FRAME_SCALE,
  MIN_FRAME_SCALE,
  patchLayer,
  setFrame,
  type Doc,
  type Frame,
  type Layer,
  type Parts,
  type PlacedLayer,
} from "../../composer/doc";
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
import { InfoIcon } from "../icons/info";
import { LoaderIcon } from "../icons/loader";

/** Room around the canvas for handles that reach past its edge, in screen pixels. */
const PAD = 30;
/** How long the stage waits before it shows its loader, so a quick load never shows one at all. */
const LOADER_DELAY_MS = 150;
/** How far above a box its turn handle floats, and how near a handle the pointer must be. */
const ROT_GAP_PX = 24;
const HANDLE_PX = 11;
const SNAP_PX = 6;
/** How far the pointer goes before a press on something becomes a drag of it. */
const DRAG_PX = 3;
/**
 * The tint over the selected part of the folder or drive: the accent lightened towards white, so
 * it shows as a change even on a part that's already the accent's colour.
 */
const partTint = (accent: string) => withAlpha(mix(accent || "#3a86ff", "#ffffff", 0.45), 0.38);

export type Backdrop = "window" | "light" | "dark" | "colour";

const handlesFor = (l: PlacedLayer): Handle[] => (l.kind === "text" || l.kind === "emoji" || l.kind === "icon" ? [...CORNERS, "rot"] : [...ALL_SIDES, "rot"]);
/** The whole folder or drive keeps its shape: it's sized from its corners and turned from its knob. */
const FRAME_HANDLES: Handle[] = [...CORNERS, "rot"];

type Drag =
  | { kind: "move"; id: string; doc0: Doc; start: Point; orig: Point; box0: Box; targets: Targets; moved: boolean; sx: number; sy: number }
  | { kind: "resize"; id: string; doc0: Doc; handle: Exclude<Handle, "rot">; box0: Box; layer0: PlacedLayer }
  | { kind: "rotate"; id: string; doc0: Doc; start: Point; box0: Box }
  | { kind: "frame-move"; doc0: Doc; frame0: Frame; start: Point; moved: boolean; sx: number; sy: number }
  | { kind: "frame-turn"; doc0: Doc; frame0: Frame; start: Point; box0: Box }
  | { kind: "frame-size"; doc0: Doc; frame0: Frame; start: Point; box0: Box };

/** The layer's new fields for a resized box. Text, emoji and icons keep their shape and change size. */
function resized(layer: PlacedLayer, box0: Box, box: Box): Record<string, number> {
  const place = { x: box.x, y: box.y };
  if (layer.kind === "text") return { ...place, size: Math.max(4, (layer.size * box.h) / box0.h) };
  if (layer.kind === "emoji" || layer.kind === "icon") return { ...place, size: Math.max(4, box.w) };
  return { ...place, w: box.w, h: box.h };
}

/**
 * The composer's canvas. It draws the design (render.ts) and shows it on the folder or flat
 * (composite.ts), and turns the pointer into changes: pick a layer, drag it (snapping to the
 * folder's middle, its tab and other layers), size it from its corners and sides, turn it from
 * the knob above it. The folder or drive is there in its parts: a click picks the part under the
 * pointer, and a drag on it moves the whole folder or drive, which its corners size and its knob
 * turns, the design on it going with it. Selection is a soft tint over the layer or the part,
 * never an outline. Until the folder and the design's pictures have loaded it shows nothing half
 * drawn: a loader, past a moment.
 */
export function ComposerStage({
  doc,
  shown = null,
  selectedId,
  onSelect,
  onPreview,
  onSettle,
  assets,
  template,
  folderLoading,
  parts,
  view,
  backdrop,
  version,
  onOpen,
  hint,
  pendingId = null,
}: {
  doc: Doc;
  /** The design as it's drawn when that isn't `doc` (with the icon library's tried icon in it). Only drawn: every change starts from `doc`. */
  shown?: Doc | null;
  selectedId: string | null;
  /** A layer of `shown` there only to be tried, outlined as not added yet and not pickable. */
  pendingId?: string | null;
  onSelect: (id: string | null) => void;
  /** A change while the pointer is still down. */
  onPreview: (doc: Doc) => void;
  /** The pointer let go: the change so far is one step. */
  onSettle: () => void;
  assets: Assets;
  template: TemplateImages | null;
  /** The folder's layers are still on their way: nothing is shown until they're here, never the design without its folder. */
  folderLoading: boolean;
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
  const upperCanvas = useRef<HTMLCanvasElement | null>(null);
  const scratch = useRef<HTMLCanvasElement | null>(null);
  const scratchSet = useRef<Scratch | null>(null);
  const [area, setArea] = useState({ w: 0, h: 0 });
  const drag = useRef<Drag | null>(null);
  /** Snapping guides, and whether they're on the folder (a layer's, turned with it) or on the canvas (the folder's own). */
  const [guides, setGuides] = useState<{ lines: Guide[]; onFolder: boolean }>({ lines: [], onFolder: true });
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

  const drawn = shown ?? doc;
  // The folder or drive in its parts: on the canvas when the design is on one and it came in parts.
  const onBase = doc.shape !== "free" && (template?.pieces.length ?? 0) > 0;
  const pivot = useMemo(() => centreOf(parts.folder), [parts.folder]);
  const frame = onBase ? (doc.base?.frame ?? HOME_FRAME) : HOME_FRAME;
  const m: Matrix = useMemo(() => frameMatrix(frame, pivot), [frame, pivot]);
  const inverse = useMemo(() => invertMatrix(m), [m]);
  /** Screen pixels per canvas unit on the folder or drive, as its frame sizes it. */
  const unit = scale * frame.scale;
  const picked = baseSelection(selectedId);
  const baseOn = onBase && picked !== null;

  // Draws on the next frame, once, however many changes came in before it.
  const raf = useRef(0);
  const latest = useRef({ doc: drawn, view, template, px, onBase, pivot, tintPart: picked?.part ?? null });
  latest.current = { doc: drawn, view, template, px, onBase, pivot, tintPart: onBase ? (picked?.part ?? null) : null };
  useEffect(() => {
    cancelAnimationFrame(raf.current);
    raf.current = requestAnimationFrame(() => {
      const c = canvas.current;
      if (!c) return;
      const { doc: d, view: v, template: tpl, px: n, onBase: parted, pivot: at, tintPart } = latest.current;
      if (c.width !== n) {
        c.width = n;
        c.height = n;
      }
      design.current ??= makeCanvas(n, n);
      scratch.current ??= makeCanvas(n, n);
      scratchSet.current ??= makeScratch();
      if (design.current.width !== n) {
        design.current.width = n;
        design.current.height = n;
      }
      renderDoc(ctx2d(design.current), d, n, assets);
      let onto: Onto | null = null;
      if (parted && tpl) {
        let upper: HTMLCanvasElement | null = null;
        if (needsUpper(tpl, d.base)) {
          upper = upperCanvas.current ??= makeCanvas(n, n);
          if (upper.width !== n) {
            upper.width = n;
            upper.height = n;
          }
          renderDoc(ctx2d(upper), d, n, assets, { from: coveringTop(d) });
        }
        const accent = wrap.current ? getComputedStyle(wrap.current).getPropertyValue("--accent").trim() : "";
        onto = {
          base: d.base,
          drive: d.shape === "drive",
          pivot: at,
          upper,
          highlight: tintPart ? { part: tintPart, color: partTint(accent) } : null,
        };
      }
      drawView(ctx2d(c), design.current, tpl, n, { ...v, onto }, scratch.current, scratchSet.current);
    });
    return () => cancelAnimationFrame(raf.current);
  }, [drawn, view, template, px, version, assets, onBase, pivot, picked?.part]);

  // What the stage is waiting for before it can show the design as it is: the folder, or a
  // picture still decoding. Past a moment, a loader stands in for the design.
  const picturesLoading = useMemo(() => assets.loading(drawn), [assets, drawn, version]);
  const waiting = folderLoading ? "folder" : picturesLoading ? "picture" : null;
  const [loader, setLoader] = useState<typeof waiting>(null);
  useEffect(() => {
    if (!waiting) {
      setLoader(null);
      return;
    }
    // Already showing, it says at once what it waits for now.
    setLoader((l) => (l ? waiting : l));
    const t = window.setTimeout(() => setLoader(waiting), LOADER_DELAY_MS);
    return () => window.clearTimeout(t);
  }, [waiting]);

  /** Where the pointer is on the canvas, in canvas units. */
  const toUnits = useCallback(
    (e: { clientX: number; clientY: number }): Point => {
      const r = wrap.current!.getBoundingClientRect();
      return { x: ((e.clientX - r.left - left) / size) * CANVAS, y: ((e.clientY - r.top - top) / size) * CANVAS };
    },
    [left, top, size],
  );
  /** A point on the canvas as it is on the folder or drive before its frame moved it: where the design's layers are. */
  const onFolder = useCallback((p: Point): Point => applyMatrix(inverse, p), [inverse]);
  /** A point on the folder or drive in screen pixels from the stage's corner. */
  const toScreen = useCallback(
    (p: Point) => {
      const q = applyMatrix(m, p);
      return { x: left + q.x * scale, y: top + q.y * scale };
    },
    [m, left, top, scale],
  );

  const placed = useMemo(() => doc.layers.filter((l): l is PlacedLayer => isPlaced(l) && !l.hidden), [doc]);
  const pending = useMemo(() => {
    const l = pendingId ? drawn.layers.find((x) => x.id === pendingId) : undefined;
    return l && isPlaced(l) ? l : null;
  }, [drawn, pendingId]);

  /** The topmost layer under a point on the folder that can be picked on the canvas. */
  const layerAt = useCallback(
    (p: Point): PlacedLayer | null => {
      for (let i = placed.length - 1; i >= 0; i--) {
        const l = placed[i];
        if (!l.locked && contains(boxOf(l, assets), p, 6 / unit)) return l;
      }
      return null;
    },
    [placed, assets, unit],
  );

  const selected = findLayer(doc, selectedId);
  const selBox = selected && isPlaced(selected) && !selected.hidden ? boxOf(selected, assets) : null;
  const gap = ROT_GAP_PX / unit;
  const radius = HANDLE_PX / unit;
  /** The whole folder or drive's box on the canvas, where its frame puts it. */
  const [fx0, fy0, fx1, fy1] = parts.folder;
  const frameBox: Box = { x: pivot.x + frame.x, y: pivot.y + frame.y, w: (fx1 - fx0) * frame.scale, h: (fy1 - fy0) * frame.scale, rotation: frame.rotation };

  /**
   * What a click at `p` (on the folder) picks there when it isn't a layer with a box: the design's
   * colour or pattern that shows there, or else the folder's or drive's own part. Null off it.
   */
  const pickOnBase = (p: Point): string | null => {
    if (!template) return null;
    const hit = partAt(template.pieces, template.size, doc.base?.parts, doc.shape === "drive", p);
    if (!hit) return null;
    const own = doc.base?.parts?.[hit.part]?.color !== undefined;
    if (hit.surface) {
      // A fill cut to the front shows only there; a part in a colour of its own hides the backgrounds under it.
      const bottom = coveringTop(doc);
      const covering = [...doc.layers]
        .map((l, i) => ({ l, i }))
        .reverse()
        .find(({ l, i }) => isCovering(l) && !l.hidden && !l.locked && !(l.kind === "fill" && l.part === "front" && doc.shape === "folder" && hit.part !== "front") && !(own && i < bottom));
      if (covering) return covering.l.id;
    }
    return partSelection(hit.part);
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    const at = toUnits(e);
    const p = onFolder(at);
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
    if (baseOn) {
      const h = hitHandle(frameBox, at, FRAME_HANDLES, HANDLE_PX / scale, ROT_GAP_PX / scale);
      if (h === "rot") return begin({ kind: "frame-turn", doc0: doc, frame0: frame, start: at, box0: frameBox });
      if (h) return begin({ kind: "frame-size", doc0: doc, frame0: frame, start: at, box0: frameBox });
    }
    const hit = layerAt(p);
    if (hit) {
      onSelect(hit.id);
      const front = centreOf(parts.front);
      const tab = centreOf(parts.tab);
      const others = boxTargets(placed.filter((l) => l.id !== hit.id).map((l) => boxOf(l, assets)));
      const targets: Targets = {
        xs: [CANVAS / 2, tab.x, fx0, fx1, ...others.xs],
        ys: [front.y, CANVAS / 2, tab.y, parts.front[1], fy0, fy1, ...others.ys],
      };
      return begin({ kind: "move", id: hit.id, doc0: doc, start: p, orig: { x: hit.x, y: hit.y }, box0: boxOf(hit, assets), targets, moved: false, sx: e.clientX, sy: e.clientY });
    }
    if (onBase) {
      const pick = pickOnBase(p);
      onSelect(pick);
      // A drag anywhere on the folder or drive moves the whole of it.
      if (pick) begin({ kind: "frame-move", doc0: doc, frame0: frame, start: at, moved: false, sx: e.clientX, sy: e.clientY });
      return;
    }
    const inside = p.x >= 0 && p.y >= 0 && p.x <= CANVAS && p.y <= CANVAS;
    const covering = inside ? [...doc.layers].reverse().find((l) => isCovering(l) && !l.hidden && !l.locked) : undefined;
    onSelect(covering?.id ?? null);
  };

  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    const at = toUnits(e);
    const p = onFolder(at);
    if (!d) {
      if (selected && isPlaced(selected) && selBox && !selected.locked) {
        const h = hitHandle(selBox, p, handlesFor(selected), radius, gap);
        if (h) {
          setCursor(cursorFor(h, selBox.rotation + frame.rotation));
          setHover(null);
          return;
        }
      }
      if (baseOn) {
        const h = hitHandle(frameBox, at, FRAME_HANDLES, HANDLE_PX / scale, ROT_GAP_PX / scale);
        if (h) {
          setCursor(cursorFor(h, frameBox.rotation));
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
    const readAt = { x: e.clientX - r.left + 14, y: e.clientY - r.top + 14 };
    if (d.kind === "move") {
      if (!d.moved && Math.hypot(e.clientX - d.sx, e.clientY - d.sy) < DRAG_PX) return;
      d.moved = true;
      let x = d.orig.x + (p.x - d.start.x);
      let y = d.orig.y + (p.y - d.start.y);
      if (!(e.metaKey || e.ctrlKey)) {
        const s = snap({ ...d.box0, x, y }, d.targets, SNAP_PX / unit);
        x += s.dx;
        y += s.dy;
        setGuides({ lines: s.guides, onFolder: true });
      } else setGuides({ lines: [], onFolder: true });
      onPreview(patchLayer(d.doc0, d.id, { x, y }));
      return;
    }
    if (d.kind === "resize") {
      const l = d.layer0;
      const corner = d.handle.length === 2;
      const keepAspect =
        l.kind === "text" || l.kind === "emoji" || l.kind === "icon" ? true : l.kind === "image" ? corner !== e.shiftKey : corner && e.shiftKey;
      const box = resizeBox(d.box0, d.handle, p, { keepAspect, fromCenter: e.altKey });
      onPreview(patchLayer(d.doc0, d.id, resized(l, d.box0, box)));
      setReadout({ text: `${Math.round(box.w)} × ${Math.round(box.h)}`, ...readAt });
      return;
    }
    if (d.kind === "rotate") {
      const rotation = rotateBy(d.box0, d.start, p, e.shiftKey);
      onPreview(patchLayer(d.doc0, d.id, { rotation }));
      setReadout({ text: `${Math.round(rotation)}°`, ...readAt });
      setCursor("grabbing");
      return;
    }
    if (d.kind === "frame-move") {
      if (!d.moved && Math.hypot(e.clientX - d.sx, e.clientY - d.sy) < DRAG_PX) return;
      d.moved = true;
      let x = d.frame0.x + (at.x - d.start.x);
      let y = d.frame0.y + (at.y - d.start.y);
      // Back where FolderSkin draws it, it settles there, unless ⌘ or Ctrl is held.
      const near = SNAP_PX / scale;
      const lines: Guide[] = [];
      if (!(e.metaKey || e.ctrlKey)) {
        if (Math.abs(x) < near) {
          x = 0;
          lines.push({ axis: "x", at: pivot.x });
        }
        if (Math.abs(y) < near) {
          y = 0;
          lines.push({ axis: "y", at: pivot.y });
        }
      }
      setGuides({ lines, onFolder: false });
      setCursor("grabbing");
      onPreview(setFrame(d.doc0, { ...d.frame0, x, y }));
      return;
    }
    if (d.kind === "frame-turn") {
      const rotation = rotateBy(d.box0, d.start, at, e.shiftKey);
      onPreview(setFrame(d.doc0, { ...d.frame0, rotation }));
      setReadout({ text: `${Math.round(rotation)}°`, ...readAt });
      setCursor("grabbing");
      return;
    }
    const from = Math.hypot(d.start.x - d.box0.x, d.start.y - d.box0.y) || 1;
    const to = Math.hypot(at.x - d.box0.x, at.y - d.box0.y);
    const s = Math.min(MAX_FRAME_SCALE, Math.max(MIN_FRAME_SCALE, Math.round(((d.frame0.scale * to) / from) * 100) / 100));
    // Back at its own size, it settles there.
    const scaleTo = Math.abs(s - 1) < 0.03 ? 1 : s;
    onPreview(setFrame(d.doc0, { ...d.frame0, scale: scaleTo }));
    setReadout({ text: `${Math.round(scaleTo * 100)}%`, ...readAt });
  };

  const end = () => {
    const d = drag.current;
    drag.current = null;
    setGuides({ lines: [], onFolder: true });
    setReadout(null);
    if (d && ((d.kind !== "move" && d.kind !== "frame-move") || d.moved)) onSettle();
  };

  /** A box on the canvas (canvas units) as the style that puts an element over it. */
  const boxStyle = (b: Box): CSSProperties => ({
    left: left + b.x * scale,
    top: top + b.y * scale,
    width: Math.max(2, b.w * scale),
    height: Math.max(2, b.h * scale),
    transform: `translate(-50%, -50%) rotate(${b.rotation}deg)`,
  });
  /** A layer's box on the folder or drive, where its frame puts it on the canvas. */
  const onCanvas = (b: Box): Box => framedBox(b, frame, pivot);
  const hovered = hover ? placed.find((l) => l.id === hover) : undefined;
  // A layer's guides run along the folder or drive, turned with it.
  const [a, b, c, d, e0, f0] = m;
  const guideTransform =
    guides.onFolder && !isHome(frame) ? `matrix(${a}, ${b}, ${c}, ${d}, ${e0 * scale + left - (a * left + c * top)}, ${f0 * scale + top - (b * left + d * top)})` : undefined;

  return (
    <div className="cmp-stage" data-backdrop={backdrop} data-waiting={folderLoading || loader ? "" : undefined} ref={wrap}>
      <canvas className="cmp-canvas" ref={canvas} style={{ left, top, width: size, height: size }} aria-hidden="true" />
      {loader && (
        <div className="cmp-stage-loader" role="status">
          <LoaderIcon size={22} />
          <span>{loader === "folder" ? (view.shape === "drive" ? t("composer.stage.driveLoading") : t("composer.stage.folderLoading")) : t("composer.stage.pictureLoading")}</span>
        </div>
      )}
      {hint && (
        <p className="cmp-stage-note" role="note">
          <InfoIcon size={14} />
          <span>{hint}</span>
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
          const hit = layerAt(onFolder(toUnits(e)));
          if (hit) onOpen(hit);
        }}
      >
        {baseOn && (
          <>
            <div className={picked?.part ? "cmp-sel is-frame" : "cmp-sel"} style={boxStyle(frameBox)} />
            {FRAME_HANDLES.map((h) => {
              const q = handlePoint(frameBox, h, ROT_GAP_PX / scale);
              return (
                <span key={h} className={h === "rot" ? "cmp-knob is-rot" : "cmp-knob"} style={{ left: left + q.x * scale, top: top + q.y * scale }} aria-hidden="true">
                  {h === "rot" && <RotateCwIcon size={11} />}
                </span>
              );
            })}
          </>
        )}
        {hovered && <div className="cmp-hover" style={boxStyle(onCanvas(boxOf(hovered, assets)))} />}
        {pending && <div className="cmp-pending" style={boxStyle(onCanvas(boxOf(pending, assets)))} aria-hidden="true" />}
        {selBox && selected && isPlaced(selected) && (
          <>
            <div className={selected.locked ? "cmp-sel is-locked" : "cmp-sel"} style={boxStyle(onCanvas(selBox))} />
            {!selected.locked &&
              handlesFor(selected).map((h) => {
                const q = toScreen(handlePoint(selBox, h, gap));
                return (
                  <span key={h} className={h === "rot" ? "cmp-knob is-rot" : "cmp-knob"} style={{ left: q.x, top: q.y }} aria-hidden="true">
                    {h === "rot" && <RotateCwIcon size={11} />}
                  </span>
                );
              })}
          </>
        )}
        <div className="cmp-guides" style={guideTransform ? { transform: guideTransform } : undefined}>
          {guides.lines.map((g) =>
            g.axis === "x" ? (
              <span key={`x${g.at}`} className="cmp-guide is-x" style={{ left: left + g.at * scale, top, height: size }} />
            ) : (
              <span key={`y${g.at}`} className="cmp-guide is-y" style={{ top: top + g.at * scale, left, width: size }} />
            ),
          )}
        </div>
        {readout && (
          <span className="cmp-readout" style={{ left: readout.x, top: readout.y }}>
            {readout.text}
          </span>
        )}
      </div>
    </div>
  );
}
