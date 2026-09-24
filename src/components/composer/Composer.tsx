import { useCallback, useEffect, useImperativeHandle, useLayoutEffect, useMemo, useReducer, useRef, useState, type CSSProperties, type ReactNode, type Ref, type RefObject } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errorMessage, type ComposerImage, type Skin } from "../../lib/tauri";
import { isTauri } from "../../lib/devMock";
import { IMAGE_EXTENSIONS } from "../../lib/files";
import { fileBrowser, keys, localOs } from "../../lib/platform";
import { cleanName, clip as clipName, MAX_NAME_CHARS } from "../../lib/names";
import type { DragInfo, Folder } from "../../state/dropzone";
import { applyLabel, folders as folderCount, tooMany, type Subfolders, type TreeProgress } from "../../lib/tree";
import type { ToastTone } from "../../hooks/useToasts";
import { Assets, ctx2d, makeCanvas } from "../../composer/assets";
import { canvasPng } from "../../composer/body";
import { inkOn, luminance } from "../../composer/color";
import { loadTemplate, type TemplateImages, type View } from "../../composer/composite";
import {
  addLayer,
  backgroundColor,
  bringForward,
  bringToFront,
  centreOf,
  cloneLayer,
  coveringTop,
  duplicateLayer,
  emptyDoc,
  fallbackParts,
  findLayer,
  imageBox,
  indexOf,
  isPlaced,
  makeEmoji,
  makeFill,
  makeIcon,
  makeImage,
  makePattern,
  makeShape,
  makeText,
  mapLayer,
  moveLayer,
  parseDoc,
  patchLayer,
  refit,
  removeLayer,
  sendBackward,
  sendToBack,
  solid,
  suggestName,
  type Doc,
  type FolderStyle,
  type IconDrawing,
  type IconLayer,
  type IconLook,
  type Layer,
  type Parts,
  type PlacedLayer,
  type PatternKind,
  type ShapeKind,
} from "../../composer/doc";
import { freeSpot, placeIcon } from "../../composer/geometry";
import { canRedo, canUndo, historyReducer, startHistory } from "../../composer/history";
import { boxOf, renderDoc } from "../../composer/render";
import { TEMPLATES, type Picture } from "../../composer/templates";
import { Confirm } from "../Confirm";
import { ComposerInspector, type Patch } from "./ComposerInspector";
import { ComposerLayers } from "./ComposerLayers";
import { IconButton } from "./controls";
import { LAYERS_HEIGHT, MIN_LAYERS, MIN_SETTINGS, Panel, Resizer, usePanels } from "./SidePanels";
import { CopyIcon } from "../icons/copy";
import { ComposerStage, type Backdrop } from "./ComposerStage";
import { PictureMenu } from "./PictureMenu";
import { EmojiPicker, PatternGrid, ShapeGrid } from "./pickers";
import { Popover } from "./Popover";
import { NewDesign, type Start } from "./NewDesign";
import { LookSwitch } from "../LookSwitch";
import { getLook } from "../../state/look";
import { IconLibrary, PREVIEW_ID } from "./IconLibrary";
import { Segmented } from "./controls";
import { ImageIcon } from "../icons/image";
import { LoaderIcon } from "../icons/loader";
import {
  FolderIcon,
  ChevronDownIcon,
  ChevronUpIcon,
  EllipsisIcon,
  InfoCircleIcon,
  LayoutTemplateIcon,
  PaintBucketIcon,
  RedoIcon,
  TrashIcon,
  ShapesIcon,
  SmileIcon,
  StickerIcon,
  TypeIcon,
  UndoIcon,
  WavesIcon,
} from "../icons/composer";

/** Something the rest of the app asked the composer to open. */
export type ComposerRequest = { kind: "edit" | "remix"; skin: Skin; nonce: number };
/** What the app can ask of a mounted composer. */
export type ComposerHandle = { addImagePath: (path: string) => void };

type Editing = { skinId: string; tags: string[] };
type Draft = { doc: Doc; name: string; nameTouched: boolean; editing: Editing | null; dirty: boolean };

const DRAFT_KEY = "folderskin.composer.draft.v1";
const VIEW_KEY = "folderskin.composer.view";
/** Undo's and redo's shortcuts, for their tooltips. */
const MOD_KEYS = { undo: keys("Z"), redo: localOs() === "macos" ? keys("Z", { shift: true }) : keys("Y") };
const fileBrowserName = () => fileBrowser(localOs());

/** How new icons look, as last chosen in the icon library. */
const LOOK_KEY = "folderskin.composer.iconLook";

function loadLook(): IconLook {
  try {
    const v = localStorage.getItem(LOOK_KEY);
    return v === "flat" || v === "original" ? v : "emboss";
  } catch {
    return "emboss";
  }
}
/** The size the saved design is drawn at: the compositor's master size, so nothing is scaled up. */
const SAVE_PX = 2048;
const PREVIEW_SIZES = [128, 64, 32];

function loadDraft(): Draft | null {
  try {
    const raw = JSON.parse(localStorage.getItem(DRAFT_KEY) ?? "null") as Partial<Draft> | null;
    const doc = raw ? parseDoc(raw.doc) : null;
    if (!raw || !doc) return null;
    const editing = raw.editing && typeof raw.editing.skinId === "string" ? { skinId: raw.editing.skinId, tags: Array.isArray(raw.editing.tags) ? raw.editing.tags : [] } : null;
    return { doc, name: typeof raw.name === "string" ? raw.name : "", nameTouched: raw.nameTouched === true, editing, dirty: raw.dirty !== false };
  } catch {
    return null;
  }
}

function loadView(): { backdrop: Backdrop } {
  try {
    const v = JSON.parse(localStorage.getItem(VIEW_KEY) ?? "{}") as { backdrop?: unknown };
    const backdrops: Backdrop[] = ["window", "light", "dark", "colour"];
    return { backdrop: backdrops.includes(v.backdrop as Backdrop) ? (v.backdrop as Backdrop) : "window" };
  } catch {
    return { backdrop: "window" };
  }
}

/** Every colour a design uses, for the picker's "In this design" row. */
function colorsOf(doc: Doc): string[] {
  const out = new Set<string>();
  const paint = (p: { type: string; color?: string; stops?: { color: string }[] }) => {
    if (p.type === "solid" && p.color) out.add(p.color);
    for (const s of p.stops ?? []) out.add(s.color);
  };
  for (const l of doc.layers) {
    if (l.kind === "fill" || l.kind === "text" || l.kind === "shape" || l.kind === "icon") paint(l.paint);
    if (l.kind === "pattern") out.add(l.color);
    if ((l.kind === "text" || l.kind === "shape") && l.stroke) out.add(l.stroke.color);
    if (isPlaced(l) && l.edge) out.add(l.edge.color);
  }
  return [...out];
}

/** A picture pasted or dropped as a file, shrunk to at most 2048 px, as a picture layer takes it. */
function readPicture(file: Blob): Promise<ComposerImage> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error("couldn't read that picture"));
    reader.onload = () => {
      const img = new Image();
      img.onerror = () => reject(new Error("couldn't read that picture"));
      img.onload = () => {
        const k = Math.min(1, 2048 / Math.max(img.naturalWidth, img.naturalHeight));
        const w = Math.max(1, Math.round(img.naturalWidth * k));
        const h = Math.max(1, Math.round(img.naturalHeight * k));
        const c = makeCanvas(w, h);
        const g = ctx2d(c);
        g.drawImage(img, 0, 0, w, h);
        // A quick look for transparency, on a small copy.
        const probe = makeCanvas(48, 48);
        const pg = ctx2d(probe);
        pg.drawImage(img, 0, 0, 48, 48);
        const data = pg.getImageData(0, 0, 48, 48).data;
        let alpha = false;
        for (let i = 3; i < data.length; i += 4) if (data[i] < 250) alpha = true;
        resolve({ url: alpha ? c.toDataURL("image/png") : c.toDataURL("image/jpeg", 0.9), width: w, height: h, name: "Pasted picture", alpha });
      };
      img.src = reader.result as string;
    };
    reader.readAsDataURL(file);
  });
}

const asPicture = (img: ComposerImage): Picture => ({ src: img.url, width: img.width, height: img.height, alpha: img.alpha });

/** One of the toolbar's tools: an action, or a panel of choices (emoji, shapes) that opens under it. */
type ToolDef = { label: string; icon: ReactNode; hint: string; onClick?: () => void; popover?: (close: () => void) => ReactNode; width?: number };

function Tool({ tool }: { tool: ToolDef }) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const { label, icon, hint, onClick, popover, width = 300 } = tool;
  return (
    <>
      <button
        type="button"
        className={anchor ? "cmp-tool is-open" : "cmp-tool"}
        aria-haspopup={popover ? "dialog" : undefined}
        aria-expanded={popover ? anchor !== null : undefined}
        data-tip={hint}
        data-tip-side="bottom"
        onClick={(e) => (popover ? setAnchor(anchor ? null : e.currentTarget) : onClick?.())}
      >
        {icon}
        <span className="cmp-tool-label">{label}</span>
      </button>
      {anchor && popover && (
        <Popover anchor={anchor} onClose={() => setAnchor(null)} width={width} label={label}>
          {popover(() => setAnchor(null))}
        </Popover>
      )}
    </>
  );
}

/** The ⋯ at the end of a toolbar too narrow for every tool: the rest, in a menu. A tool with a panel opens it in the same place. */
function MoreTools({ tools }: { tools: ToolDef[] }) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const [inner, setInner] = useState<ToolDef | null>(null);
  const close = () => {
    setAnchor(null);
    setInner(null);
  };
  return (
    <>
      <button
        type="button"
        className={anchor ? "cmp-tool is-open" : "cmp-tool"}
        aria-haspopup="menu"
        aria-expanded={anchor !== null}
        aria-label="more tools"
        data-tip={tools.map((t) => t.label).join(", ")}
        data-tip-side="bottom"
        onClick={(e) => (anchor ? close() : setAnchor(e.currentTarget))}
      >
        <EllipsisIcon size={16} />
        <span className="cmp-tool-label">More</span>
      </button>
      {anchor && (
        <Popover anchor={anchor} onClose={close} width={inner ? (inner.width ?? 300) : 200} label={inner ? inner.label : "More tools"} align="end">
          {inner?.popover ? (
            inner.popover(close)
          ) : (
            <div className="cmp-menu" role="menu">
              {tools.map((t) => (
                <button
                  key={t.label}
                  type="button"
                  role="menuitem"
                  className="menu-item"
                  onClick={() => {
                    if (t.popover) setInner(t);
                    else {
                      close();
                      t.onClick?.();
                    }
                  }}
                >
                  {t.icon}
                  {t.label}
                </button>
              ))}
            </div>
          )}
        </Popover>
      )}
    </>
  );
}

/** Room the ⋯ takes, and the space between tools (composer.css). */
const MORE_WIDTH = 50;
const TOOL_GAP = 1;

/**
 * The tools that fit, then ⋯ with the rest. Each tool's width is measured once from an unseen copy
 * of the row, so the row can be fitted to its room as the window changes without a tool ever
 * being drawn half cut off.
 */
function Tools({ tools }: { tools: ToolDef[] }) {
  const row = useRef<HTMLDivElement>(null);
  const ruler = useRef<HTMLDivElement>(null);
  const [fit, setFit] = useState(tools.length);
  useLayoutEffect(() => {
    const el = row.current;
    if (!el || !ruler.current) return;
    const measure = () => {
      // Read each time: the tools' own widths change once the app's font has loaded.
      const widths = [...(ruler.current?.children ?? [])].map((c) => (c as HTMLElement).offsetWidth);
      const room = el.clientWidth;
      const total = widths.reduce((s, w) => s + w, 0) + TOOL_GAP * (widths.length - 1);
      if (total <= room) return setFit(widths.length);
      let used = MORE_WIDTH;
      let n = 0;
      while (n < widths.length && used + widths[n] + TOOL_GAP <= room) used += widths[n++] + TOOL_GAP;
      setFit(n);
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    ro.observe(ruler.current);
    return () => ro.disconnect();
  }, [tools.length]);
  const shown = tools.slice(0, fit);
  const rest = tools.slice(fit);
  return (
    <div className="cmp-tools" role="toolbar" aria-label="add to the design" ref={row}>
      {shown.map((t) => (
        <Tool key={t.label} tool={t} />
      ))}
      {rest.length > 0 && <MoreTools tools={rest} />}
      <div className="cmp-tools-ruler" ref={ruler} aria-hidden="true">
        {tools.map((t) => (
          <span key={t.label} className="cmp-tool">
            {t.icon}
            <span className="cmp-tool-label">{t.label}</span>
          </span>
        ))}
      </div>
    </div>
  );
}

/**
 * Whether the canvas bar has room for its backdrops and its size previews beside its switches.
 * The previews give way first, then the backdrops, so the bar never draws one cut off at the
 * panel's edge, as the tools above move into ⋯. Each width is kept from when it was last shown,
 * so it comes back as soon as there is room for it again.
 */
function useBarRoom(bar: RefObject<HTMLDivElement | null>, backdrops: RefObject<HTMLDivElement | null>, sizes: RefObject<HTMLDivElement | null>, layout: unknown) {
  const [room, setRoom] = useState({ backdrops: true, sizes: true });
  useLayoutEffect(() => {
    const el = bar.current;
    if (!el) return;
    const kept = new WeakMap<Element, number>();
    const width = (k: HTMLElement) => {
      if (!k.hidden && k.offsetWidth > 0) kept.set(k, k.offsetWidth);
      return kept.get(k) ?? 0;
    };
    const measure = () => {
      const style = getComputedStyle(el);
      const gap = parseFloat(style.columnGap) || 0;
      const inner = el.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
      const b = backdrops.current;
      const s = sizes.current;
      const fixed = ([...el.children] as HTMLElement[]).filter((k) => k !== b && k !== s);
      const base = fixed.reduce((sum, k) => sum + width(k), 0) + gap * Math.max(0, fixed.length - 1);
      const withBackdrops = base + (b ? width(b) + gap : 0);
      const next = { backdrops: withBackdrops <= inner, sizes: withBackdrops + (s ? width(s) + gap : 0) <= inner };
      setRoom((r) => (r.backdrops === next.backdrops && r.sizes === next.sizes ? r : next));
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    for (const k of el.children) ro.observe(k);
    return () => ro.disconnect();
  }, [bar, backdrops, sizes, layout]);
  return room;
}

/** How to get around the canvas, behind the info button rather than taking room in the panel. */
function TipsButton() {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  return (
    <>
      <button
        type="button"
        className={anchor ? "cmp-icon-btn is-on" : "cmp-icon-btn"}
        aria-label="tips and shortcuts"
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        data-tip="Tips and shortcuts"
        data-tip-side="bottom"
        onClick={(e) => setAnchor(anchor ? null : e.currentTarget)}
      >
        <InfoCircleIcon size={16} />
      </button>
      {anchor && (
        <Popover anchor={anchor} onClose={() => setAnchor(null)} width={300} label="Tips and shortcuts" align="end">
          <p className="cmp-pop-title">Tips and shortcuts</p>
          <ul className="cmp-tips">
            <li>Click the folder to change its colour.</li>
            <li>Double-click words to edit them, an emoji to swap it, or an icon to pick another.</li>
            <li>Drag a corner to resize, the round knob to turn. Hold ⇧ for even steps.</li>
            <li>Hold {localOs() === "macos" ? "⌘" : "Ctrl"} while dragging to stop things snapping into place.</li>
            <li>Drop or paste a picture straight onto the folder.</li>
            <li>
              {keys("Z")} undoes, {keys("D")} duplicates, Delete removes the selected layer.
            </li>
          </ul>
        </Popover>
      )}
    </>
  );
}

const BACKDROPS: { id: Backdrop; label: string }[] = [
  { id: "window", label: "Window" },
  { id: "light", label: "Light desktop" },
  { id: "dark", label: "Dark desktop" },
  { id: "colour", label: "Colourful wallpaper" },
];

/**
 * The composer: design a skin on a canvas and save it to the library, or straight onto the
 * chosen folder. It takes the two islands (the canvas, and the layers with their settings); the
 * folder panel steps aside while it's open and comes back with the rest of the app.
 *
 * It stays mounted once opened, so a design survives a visit to the library; it's also kept in
 * this computer's storage, so it survives a restart until it's saved or started over.
 */
/** What an apply from the composer did, for its toast. */
export type ApplyOutcome = {
  /** The skin went on: to every folder, or to some when a run over subfolders stopped or had failures. */
  ok: boolean;
  /** What to say, when the composer can't say it itself; left out when the apply was cancelled. */
  message?: string;
  tone?: ToastTone;
  action?: { label: string; run: () => void };
};

export function Composer({
  ref,
  active,
  skins,
  folder,
  folderIcon,
  applying,
  drag,
  subfolders,
  includeSubfolders,
  onIncludeSubfolders,
  progress,
  stopping,
  onStop,
  onChooseFolder,
  onSaved,
  onApply,
  request,
  toast,
}: {
  ref?: Ref<ComposerHandle>;
  /** In view: its keyboard shortcuts and previews only run then. */
  active: boolean;
  skins: Skin[];
  folder: Folder | null;
  folderIcon: string | null | undefined;
  applying: boolean;
  /** What is being dragged over the window: a picture becomes a layer, a folder the one to apply to. */
  drag: DragInfo | null;
  /** The folders inside the chosen one, once counted. */
  subfolders: Subfolders | null;
  /** Save & apply reaches them too; the same switch as the folder panel's. */
  includeSubfolders: boolean;
  onIncludeSubfolders: (on: boolean) => void;
  /** How far an apply over the folder and its subfolders has got. */
  progress: TreeProgress | null;
  stopping: boolean;
  onStop: () => void;
  onChooseFolder: () => void;
  /** A design was saved, as a new skin or in place of `replaced`. */
  onSaved: (skin: Skin, replaced: string | null) => void;
  /** Applies a saved design to the chosen folder, and its subfolders when they're included. */
  onApply: (skin: Skin) => Promise<ApplyOutcome>;
  request: ComposerRequest | null;
  toast: (text: string, opts?: { tone?: ToastTone; action?: { label: string; run: () => void } }) => void;
}) {
  const assets = useMemo(() => new Assets(), []);
  const [version, setVersion] = useState(0);
  useEffect(() => assets.onChange(() => setVersion((v) => v + 1)), [assets]);
  useEffect(() => {
    const fonts = (document as Document & { fonts?: FontFaceSet }).fonts;
    if (!fonts) return;
    const changed = () => assets.fontsChanged();
    fonts.addEventListener?.("loadingdone", changed);
    void fonts.ready.then(changed);
    return () => fonts.removeEventListener?.("loadingdone", changed);
  }, [assets]);

  const draft = useMemo(loadDraft, []);
  const [history, dispatch] = useReducer(historyReducer, undefined, () => {
    // A first design is on the folder chosen in the folder panel: the Mac's unless it says Windows'.
    const style: FolderStyle = getLook();
    return startHistory(draft?.doc ?? { ...TEMPLATES[0].make(fallbackParts(style)), style });
  });
  const doc = history.present;

  // Each folder's layers, from Rust, the first time a design is on that folder; null when they
  // didn't load, and the design is shown by itself.
  const [templates, setTemplates] = useState<Partial<Record<FolderStyle, { images: TemplateImages; parts: Parts } | null>>>({});
  const asked = useRef(new Set<FolderStyle>());
  useEffect(() => {
    const style = doc.style;
    if (asked.current.has(style)) return;
    asked.current.add(style);
    api
      .composerTemplate(style)
      .then(async (t) => {
        const images = await loadTemplate(t);
        setTemplates((all) => ({ ...all, [style]: { images, parts: t.parts } }));
      })
      .catch((e) => {
        asked.current.delete(style);
        setTemplates((all) => ({ ...all, [style]: null }));
        toast(`The folder preview didn't load: ${errorMessage(e)}`, { tone: "danger" });
      });
  }, [doc.style, toast]);
  const template = templates[doc.style] ?? null;
  /** The folder the design is on hasn't loaded yet: the stage waits for it rather than show the design without it. */
  const folderLoading = templates[doc.style] === undefined;
  const parts = template?.parts ?? fallbackParts(doc.style);
  /** The design as it was when it was last saved, opened or started: anything else is a change. */
  const [baseline, setBaseline] = useState<Doc>(() => (draft?.dirty ? emptyDoc() : history.present));
  const dirty = doc !== baseline;
  const [selectedId, setSelectedId] = useState<string | null>(null);
  /** The "Start a new design" dialog: on the first visit, and whenever New is pressed. */
  const [starting, setStarting] = useState(!draft);
  const [name, setName] = useState(draft?.name ?? "");
  const [nameTouched, setNameTouched] = useState(draft?.nameTouched ?? false);
  const [editing, setEditing] = useState<Editing | null>(draft?.editing ?? null);
  const [saving, setSaving] = useState<"save" | "copy" | "apply" | null>(null);
  const [confirm, setConfirm] = useState<{ title: string; text: string; action: string; run: () => void } | null>(null);
  const [view, setView] = useState(loadView);
  const [previews, setPreviews] = useState<string[]>([]);
  /** What the side island shows: the layers and their settings, or the icon library. */
  const [side, setSide] = useState<"layers" | "icons">("layers");
  /** How the next icon added looks; the selected icon's own look is shown and changed in its place. */
  const [iconLook, setIconLook] = useState<IconLook>(loadLook);
  /** The library stays mounted once opened, so its search and scroll survive a look at the layers. */
  const [iconsOpened, setIconsOpened] = useState(false);
  useEffect(() => {
    if (side === "icons") setIconsOpened(true);
  }, [side]);
  /** The Replace button whose picture menu is open. */
  const [replaceAnchor, setReplaceAnchor] = useState<HTMLElement | null>(null);
  const textRef = useRef<HTMLTextAreaElement>(null);
  const clip = useRef<Layer | null>(null);
  const barRef = useRef<HTMLDivElement>(null);
  const backdropsRef = useRef<HTMLDivElement>(null);
  const sizesRef = useRef<HTMLDivElement>(null);
  // The look switch is only there on the folder skeleton, so the room is looked at again then.
  const barRoom = useBarRoom(barRef, backdropsRef, sizesRef, doc.shape);

  // A saved design that's gone from the library (deleted) is a new design again.
  useEffect(() => {
    if (editing && skins.length > 0 && !skins.some((s) => s.id === editing.skinId)) setEditing(null);
  }, [skins, editing]);

  const selected = findLayer(doc, selectedId);
  useEffect(() => {
    if (selectedId && !selected) setSelectedId(null);
  }, [selectedId, selected]);

  const commit = useCallback((next: Doc, key?: string) => dispatch({ type: "commit", doc: next, key }), []);
  const latestDoc = useRef(doc);
  latestDoc.current = doc;

  // ---- keeping the design ----
  useEffect(() => {
    const t = window.setTimeout(() => {
      try {
        const d: Draft = { doc, name, nameTouched, editing, dirty };
        localStorage.setItem(DRAFT_KEY, JSON.stringify(d));
      } catch {
        // A design with big pictures can be too much for storage. It still lasts until the app
        // quits; an older copy mustn't come back in its place after a restart.
        try {
          localStorage.removeItem(DRAFT_KEY);
        } catch {
          // Nothing kept, nothing to forget.
        }
      }
    }, 700);
    return () => window.clearTimeout(t);
  }, [doc, name, nameTouched, editing, dirty]);

  useEffect(() => {
    try {
      localStorage.setItem(VIEW_KEY, JSON.stringify(view));
    } catch {
      // Only a preference.
    }
  }, [view]);

  // ---- starting ----
  const reset = useCallback((next: Doc, opts: { editing: Editing | null; name: string; named: boolean }) => {
    dispatch({ type: "reset", doc: next });
    setBaseline(next);
    setSelectedId(null);
    setEditing(opts.editing);
    setName(opts.name);
    setNameTouched(opts.named);
    setStarting(false);
    // The old design's icons mustn't show while the new one's are drawn.
    setPreviews([]);
    assets.prune([next]);
  }, [assets]);

  /** Forgets the stored draft, for a fresh start that shouldn't come back after a restart. */
  const forgetDraft = useCallback(() => {
    try {
      localStorage.removeItem(DRAFT_KEY);
    } catch {
      // The next autosave writes over it anyway.
    }
  }, []);

  /** Asks before throwing away changes that aren't saved. */
  const guard = useCallback(
    (what: string, run: () => void | Promise<void>) => {
      if (!dirty) return run();
      setConfirm({
        title: "Start again?",
        text: `${what} replaces the design you're working on, and its changes aren't saved.`,
        action: "Replace it",
        run,
      });
    },
    [dirty],
  );

  const choosePicture = useCallback(async (): Promise<ComposerImage | null> => {
    try {
      if (!isTauri()) return await api.composerImage("mock");
      const picked = await open({ multiple: false, title: "Choose a picture", filters: [{ name: "Pictures", extensions: IMAGE_EXTENSIONS }] }).catch(() => null);
      if (typeof picked !== "string") return null;
      return await api.composerImage(picked);
    } catch (e) {
      toast(`Couldn't add that picture: ${errorMessage(e)}`, { tone: "danger" });
      return null;
    }
  }, [toast]);

  /**
   * Starts again from the dialog's choice. The dialog has already said the design in progress
   * goes if it isn't saved, so nothing asks a second time.
   */
  const start = useCallback(
    async (choice: Start) => {
      // A new design stays on the folder the last one was on.
      const style = latestDoc.current.style;
      if (choice.kind === "empty") {
        reset(emptyDoc(choice.shape, style), { editing: null, name: "", named: false });
        forgetDraft();
        return;
      }
      let picture: Picture | undefined;
      if (choice.template.photo) {
        const img = await choosePicture();
        // No picture chosen: stay in the dialog to pick again.
        if (!img) return;
        picture = asPicture(img);
      }
      reset({ ...choice.template.make(parts, picture), style }, { editing: null, name: "", named: false });
      forgetDraft();
    },
    [choosePicture, forgetDraft, parts, reset],
  );

  // Edit a saved design, or remix any skin, when the app asks.
  const handled = useRef(0);
  useEffect(() => {
    if (!request || request.nonce === handled.current) return;
    handled.current = request.nonce;
    const { skin } = request;
    const load = async () => {
      try {
        if (request.kind === "edit") {
          const saved = parseDoc(await api.composerDesign(skin.id));
          if (saved) return reset(saved, { editing: { skinId: skin.id, tags: skin.tags }, name: skin.name, named: true });
        }
        const img = await api.composerSkinImage(skin.id);
        let next: Doc;
        if (skin.kind === "folder") {
          // A finished folder keeps its own shape: it becomes a free icon with the picture fitted in, as the app applies it.
          const k = Math.min(1024 / img.width, 1024 / img.height);
          next = { ...emptyDoc("free", latestDoc.current.style), layers: [makeImage(img.url, img.width, img.height, { x: 512, y: 512, w: img.width * k, h: img.height * k })] };
        } else {
          next = { ...emptyDoc("folder", latestDoc.current.style), layers: [makeImage(img.url, img.width, img.height, imageBox(img.width, img.height, parts, true))] };
        }
        reset(next, { editing: null, name: `${skin.name} remix`, named: true });
      } catch (e) {
        toast(`Couldn't open ${clipName(skin.name)}: ${errorMessage(e)}`, { tone: "danger" });
      }
    };
    if (editing?.skinId === skin.id && request.kind === "edit") return;
    guard(request.kind === "edit" ? `Editing ${clipName(skin.name)}` : `Remixing ${clipName(skin.name)}`, () => void load());
  }, [request, editing, guard, parts, reset, toast]);

  // ---- adding ----
  const front = centreOf(parts.front);
  const bg = backgroundColor(doc);
  const ink = bg ? inkOn(bg) : "#ffffff";
  const accent = bg && luminance(bg) > 0.5 ? "#3a86ff" : "#ffffff";

  const add = useCallback(
    (layer: Layer, index?: number) => {
      commit(addLayer(latestDoc.current, layer, index));
      setSelectedId(layer.id);
    },
    [commit],
  );

  const addText = () => {
    const text = makeText("Your words", front.x, front.y, ink);
    // Beside what's there already, as an icon goes, not over a label's own words.
    const { w, h } = boxOf(text, assets);
    const at = freeSpot(parts.front, front, w, h, taken);
    add(at ? { ...text, x: at.x, y: at.y } : text);
    window.setTimeout(() => {
      textRef.current?.focus();
      textRef.current?.select();
    }, 60);
  };
  const addEmoji = (char: string) => add(makeEmoji(char, front.x, front.y + 6));
  const addShape = (shape: ShapeKind) => add(makeShape(shape, front.x, front.y, accent));
  const addPattern = (pattern: PatternKind) => add(makePattern(pattern, bg && luminance(bg) > 0.6 ? "#1b1f2733" : "#ffffff59"), coveringTop(latestDoc.current));
  const addBackground = () => {
    const first = latestDoc.current.layers[0];
    if (first?.kind === "fill") setSelectedId(first.id);
    else add(makeFill(solid("#3a86ff")), 0);
  };
  const addPicture = useCallback(
    (img: ComposerImage) => {
      const d = latestDoc.current;
      // A photo covers the folder, above the background; a cut-out (a logo) sits on top, on the front.
      const cover = !img.alpha;
      const layer = makeImage(img.url, img.width, img.height, imageBox(img.width, img.height, parts, cover));
      add(layer, cover ? coveringTop(d) : d.layers.length);
    },
    [add, parts],
  );
  const addPictureFile = async () => {
    const img = await choosePicture();
    if (img) addPicture(img);
  };
  const addSkinPicture = async (skin: Skin, into?: string) => {
    try {
      const img = await api.composerSkinImage(skin.id);
      if (into) replacePicture(into, img);
      else addPicture({ ...img, alpha: skin.kind === "folder" || img.alpha });
    } catch (e) {
      toast(`Couldn't use ${clipName(skin.name)}: ${errorMessage(e)}`, { tone: "danger" });
    }
  };
  /** Shows the icon library, for icon layer `id` when there is one: the library then works on it. */
  const openIcons = useCallback((id: string | null) => {
    if (id) setSelectedId(id);
    setSide("icons");
  }, []);

  /** The icon the library swaps: the selected one, when it's an icon. With none, the library adds. */
  const iconTarget: IconLayer | null = selected?.kind === "icon" ? selected : null;
  /** The design with the icon being tried in it, shown on the canvas until it's kept or let go. */
  const [iconPreview, setIconPreview] = useState<Doc | null>(null);

  /**
   * Where the next icon goes: the front's middle, or else the first spot beside it that nothing on
   * the front already covers, so icons added one after another sit side by side rather than on top
   * of each other. The first is the size of a folder's symbol, the ones after a little smaller.
   */
  /** What a new icon or text should keep clear of: the icons, emoji and words already there. */
  const taken = useMemo(
    () => doc.layers.filter((l): l is PlacedLayer => isPlaced(l) && !l.hidden && (l.kind === "icon" || l.kind === "emoji" || l.kind === "text")).map((l) => boxOf(l, assets)),
    [doc.layers, assets],
  );
  const iconSpot = useMemo(() => {
    const n = doc.layers.filter((l) => l.kind === "icon").length;
    return { ...placeIcon(parts.front, { x: front.x, y: front.y }, taken, n), color: ink };
  }, [doc.layers, taken, parts.front, front.x, front.y, ink]);

  /** An icon added from the library, in its free spot. Nothing is selected, so the next click tries another. */
  const addIcon = (drawing: IconDrawing) => {
    const layer = makeIcon(drawing, iconSpot.x, iconSpot.y, iconLook, iconSpot.color, iconSpot.size);
    commit(addLayer(latestDoc.current, layer));
    setSelectedId(null);
  };

  /** An icon put in the selected one's place, at its size and with its look. */
  const swapIcon = (drawing: IconDrawing) => {
    const d = latestDoc.current;
    const target = iconTarget ? findLayer(d, iconTarget.id) : null;
    if (target?.kind !== "icon") return;
    commit(
      mapLayer(d, target.id, (l) =>
        l.kind === "icon"
          ? { ...l, pack: drawing.pack, icon: drawing.icon, paths: [...drawing.paths], filled: [...(drawing.filled ?? [])], style: drawing.style, viewBox: drawing.viewBox, strokeWidth: drawing.strokeWidth, evenOdd: drawing.evenOdd === true, brand: drawing.brand, look: l.look === "original" && !drawing.brand ? "flat" : l.look }
          : l,
      ),
    );
  };

  /**
   * A look chosen in the library: the selected icon's, at once, and the next new one's. Original
   * for a selected logo is that logo's only; chosen while adding, the logos added next keep their
   * colours (icons without colours of their own come out flat).
   */
  const chooseLook = (look: IconLook) => {
    if (iconTarget && iconTarget.look !== look) commit(patchLayer(latestDoc.current, iconTarget.id, { look }));
    if (look === "original" && iconTarget) return;
    setIconLook(look);
    try {
      localStorage.setItem(LOOK_KEY, look);
    } catch {
      // Only a preference.
    }
  };

  const replacePicture = (id: string, img: ComposerImage) => {
    commit(
      mapLayer(latestDoc.current, id, (l) =>
        l.kind === "image" ? { ...l, src: img.url, iw: img.width, ih: img.height, h: (l.w * img.height) / img.width } : l,
      ),
    );
  };

  useImperativeHandle(
    ref,
    () => ({
      addImagePath: (path: string) => {
        api
          .composerImage(path)
          .then(addPicture)
          .catch((e) => toast(`Couldn't add that picture: ${errorMessage(e)}`, { tone: "danger" }));
      },
    }),
    [addPicture, toast],
  );

  // ---- changing ----
  const patch = useCallback(
    (p: Patch, key?: string) => {
      if (!selectedId) return;
      commit(patchLayer(latestDoc.current, selectedId, p), key);
    },
    [selectedId, commit],
  );

  /** Deletes a layer. When it was the selected one, the one below it is selected next, so pressing Delete again keeps going. */
  const removeById = useCallback(
    (id: string) => {
      const d = latestDoc.current;
      const i = indexOf(d, id);
      if (i < 0) return;
      commit(removeLayer(d, id));
      if (id !== selectedId) return;
      const next = d.layers[i - 1] ?? d.layers[i + 1];
      setSelectedId(next && next.id !== id ? next.id : null);
    },
    [selectedId, commit],
  );
  const remove = useCallback(() => {
    if (selectedId) removeById(selectedId);
  }, [selectedId, removeById]);

  // ---- the side's panels ----
  const [panels, setPanels] = usePanels();
  const layersPanel = useRef<HTMLElement>(null);
  const settingsPanel = useRef<HTMLElement>(null);
  const bothOpen = panels.layers && panels.settings;
  /** Where the bar between the panels is, and how far it can go: the settings keep room for a few controls. */
  const measureLayers = useCallback(() => {
    const height = layersPanel.current?.offsetHeight ?? panels.height;
    const settings = settingsPanel.current?.offsetHeight ?? MIN_SETTINGS;
    return { height, min: MIN_LAYERS, max: Math.max(MIN_LAYERS, height + settings - MIN_SETTINGS) };
  }, [panels.height]);

  const duplicate = useCallback(() => {
    if (!selectedId) return;
    const r = duplicateLayer(latestDoc.current, selectedId);
    if (r.id) {
      commit(r.doc);
      setSelectedId(r.id);
    }
  }, [selectedId, commit]);

  // ---- keyboard, paste ----
  useEffect(() => {
    if (!active) return;
    const typingIn = (t: EventTarget | null) => t instanceof Element && t.closest("input, textarea, select, [contenteditable='true']") !== null;
    // ⌘V pastes a picture from the clipboard through the paste event. A copied layer is pasted
    // there too, or here a moment later if the web view sends no paste event at all.
    let pasteLater = 0;
    const pasteLayer = () => {
      if (clip.current) add(cloneLayer(clip.current, isPlaced(clip.current) ? 28 : 0));
    };
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      const k = e.key.toLowerCase();
      if (typingIn(e.target)) return;
      // A control used the key itself: the icon grid's arrows, Enter and Space move through it and pick.
      if (e.defaultPrevented) return;
      // A dialog is open over the design: nothing here may change what's behind it.
      if (document.querySelector(".modal-backdrop")) return;
      if (mod && k === "z") {
        e.preventDefault();
        dispatch({ type: e.shiftKey ? "redo" : "undo" });
        return;
      }
      if (mod && k === "y") {
        e.preventDefault();
        dispatch({ type: "redo" });
        return;
      }
      if (mod && k === "v" && clip.current) {
        window.clearTimeout(pasteLater);
        pasteLater = window.setTimeout(pasteLayer, 120);
        return;
      }
      if (document.querySelector(".cmp-pop")) return;
      const d = latestDoc.current;
      const sel = findLayer(d, selectedId);
      if (e.key === "Escape" && sel) {
        setSelectedId(null);
        return;
      }
      if (!sel) return;
      if (e.key === "Backspace" || e.key === "Delete") {
        e.preventDefault();
        remove();
      } else if (mod && k === "d") {
        e.preventDefault();
        duplicate();
      } else if (mod && k === "c") {
        clip.current = sel;
      } else if (mod && e.key === "]") {
        e.preventDefault();
        commit(e.altKey ? bringToFront(d, sel.id) : bringForward(d, sel.id));
      } else if (mod && e.key === "[") {
        e.preventDefault();
        commit(e.altKey ? sendToBack(d, sel.id) : sendBackward(d, sel.id));
      } else if (isPlaced(sel) && !sel.locked && e.key.startsWith("Arrow") && !(e.target instanceof Element && e.target.closest(".cmp-layers"))) {
        e.preventDefault();
        const step = e.shiftKey ? 10 : 1;
        const dx = e.key === "ArrowLeft" ? -step : e.key === "ArrowRight" ? step : 0;
        const dy = e.key === "ArrowUp" ? -step : e.key === "ArrowDown" ? step : 0;
        commit(patchLayer(d, sel.id, { x: sel.x + dx, y: sel.y + dy }), `nudge:${sel.id}`);
      }
    };
    const onPaste = (e: ClipboardEvent) => {
      if (typingIn(e.target) || document.querySelector(".modal-backdrop")) return;
      window.clearTimeout(pasteLater);
      const file = Array.from(e.clipboardData?.files ?? []).find((f) => f.type.startsWith("image/"));
      if (file) {
        e.preventDefault();
        readPicture(file)
          .then(addPicture)
          .catch((err) => toast(`Couldn't paste that picture: ${errorMessage(err)}`, { tone: "danger" }));
        return;
      }
      if (clip.current) {
        e.preventDefault();
        pasteLayer();
      }
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("paste", onPaste);
    return () => {
      window.clearTimeout(pasteLater);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("paste", onPaste);
    };
  }, [active, selectedId, remove, duplicate, commit, addPicture, add, toast]);

  // ---- the icon at its real sizes, from Rust ----
  useEffect(() => {
    if (!active || starting) return;
    let live = true;
    const t = window.setTimeout(async () => {
      try {
        await assets.ready(doc);
        const c = makeCanvas(512, 512);
        renderDoc(ctx2d(c), doc, 512, assets);
        const png = await canvasPng(c);
        const urls = await api.composerPreview(doc.shape, doc.style, PREVIEW_SIZES, png);
        if (live) setPreviews(urls);
      } catch {
        // The previews are extra; the stage still shows the design.
      }
    }, 450);
    return () => {
      live = false;
      window.clearTimeout(t);
    };
  }, [doc, active, starting, assets, version]);

  // ---- saving ----
  const suggested = suggestName(doc);
  const finalName = cleanName(name) || suggested || "My design";

  const save = useCallback(
    async (mode: "save" | "copy" | "apply") => {
      if (saving) return;
      // Already saved as it is: only the apply is left to do.
      const saved = editing ? skins.find((x) => x.id === editing.skinId) : undefined;
      if (mode === "apply" && saved && !dirty) {
        setSaving("apply");
        const r = await onApply(saved);
        setSaving(null);
        if (folder && (r.ok || r.message)) toast(r.message ?? `${clipName(folder.name)} now wears ${clipName(saved.name)}`, { tone: r.tone ?? "ok", action: r.action });
        return;
      }
      setSaving(mode);
      try {
        const d = latestDoc.current;
        await assets.ready(d);
        await (document as Document & { fonts?: FontFaceSet }).fonts?.ready;
        const c = makeCanvas(SAVE_PX, SAVE_PX);
        renderDoc(ctx2d(c), d, SAVE_PX, assets);
        const png = await canvasPng(c);
        const replaces = mode === "copy" ? null : (editing?.skinId ?? null);
        const res = await api.composerSave({ name: finalName, tags: mode === "copy" ? [] : (editing?.tags ?? []), shape: d.shape, style: d.style, design: d, replaces }, png);
        onSaved(res.skin, res.replaced);
        setEditing({ skinId: res.skin.id, tags: res.skin.tags });
        setName(res.skin.name);
        setNameTouched(true);
        setBaseline(d);
        const kept = res.replaced ? `Saved the changes to ${clipName(res.skin.name)}` : `${clipName(res.skin.name)} is in Yours`;
        const r = mode === "apply" ? await onApply(res.skin) : null;
        if (r?.ok && folder) {
          toast(r.message ? `Saved. ${r.message}` : `Saved, and ${clipName(folder.name)} now wears ${clipName(res.skin.name)}`, { tone: r.tone ?? "ok", action: r.action });
        } else if (r?.message) {
          // Saved all the same; say so before what stopped the apply.
          toast(`Saved, but ${r.message.charAt(0).toLowerCase()}${r.message.slice(1)}`, { tone: "danger" });
        } else {
          // A plain save, or a Save & apply whose apply was cancelled.
          toast(kept, { tone: "ok" });
        }
      } catch (e) {
        toast(`Couldn't save the design: ${errorMessage(e)}`, { tone: "danger" });
      } finally {
        setSaving(null);
      }
    },
    [saving, editing, dirty, onApply, folder, finalName, assets, onSaved, toast, skins],
  );

  // ---- views ----
  /** The last move between the folders, so switching straight back puts the design back exactly. */
  const lastRestyle = useRef<{ from: Doc; to: Doc } | null>(null);
  const restyle = (style: FolderStyle) => {
    const d = latestDoc.current;
    if (d.style === style) return;
    const back = lastRestyle.current;
    const moved = back && back.to === d && back.from.style === style ? back.from : refit(d, fallbackParts(d.style), fallbackParts(style), style);
    lastRestyle.current = { from: d, to: moved };
    commit(moved);
  };
  // The folder skeleton is the design's shape: on, it's drawn on the folder, a Mac's or Windows';
  // off, it's a free icon, the whole picture.
  const viewOf: View = { shape: doc.shape, skeleton: doc.shape === "folder", guide: "rgba(58,134,255,0.95)" };
  const tools: ToolDef[] = [
    { label: "Text", hint: "Add words", icon: <TypeIcon size={16} />, onClick: addText },
    { label: "Icon", hint: "Add an icon from the icon library", icon: <StickerIcon size={16} />, onClick: () => openIcons(null) },
    { label: "Emoji", hint: "Add an emoji", icon: <SmileIcon size={16} />, width: 320, popover: (close) => <EmojiPicker onPick={(c) => (addEmoji(c), close())} /> },
    { label: "Shape", hint: "Add a shape", icon: <ShapesIcon size={16} />, popover: (close) => <ShapeGrid onPick={(s) => (addShape(s), close())} /> },
    {
      label: "Picture",
      hint: "Add a picture",
      icon: <ImageIcon size={16} />,
      popover: (close) => (
        <PictureMenu
          skins={skins}
          onFile={() => {
            close();
            void addPictureFile();
          }}
          onSkin={(s) => {
            close();
            void addSkinPicture(s);
          }}
        />
      ),
    },
    { label: "Pattern", hint: "Add a pattern", icon: <WavesIcon size={16} />, popover: (close) => <PatternGrid onPick={(p) => (addPattern(p), close())} /> },
    { label: "Colour", hint: "Colour the folder", icon: <PaintBucketIcon size={16} />, onClick: addBackground },
  ];
  const index = selected ? indexOf(doc, selected.id) : -1;
  const used = useMemo(() => colorsOf(doc), [doc]);
  const busy = saving !== null || applying;

  const inside = folder && includeSubfolders && subfolders ? subfolders.count : 0;
  const primary = folder
    ? {
        label: editing && !dirty ? (inside ? applyLabel(inside) : `Apply to ${clipName(folder.name, 20)}`) : inside ? `Save & apply to ${folderCount(inside + 1)}` : "Save & apply",
        mode: "apply" as const,
      }
    : editing
      ? { label: dirty ? "Save changes" : "Saved", mode: "save" as const }
      : { label: "Save to Yours", mode: "save" as const };
  const secondary = editing
    ? dirty
      ? { label: "Save as new", mode: "copy" as const }
      : null
    : folder
      ? { label: "Save", mode: "save" as const }
      : null;
  const primaryDisabled = busy || (!folder && editing !== null && !dirty);

  return (
    <>
      <section className={drag?.kind === "image" ? "island island-main cmp-main is-drop-target" : "island island-main cmp-main"} aria-label="composer" hidden={!active}>
        <span className="drop-glow" aria-hidden="true" />
        <div className="cmp-toolbar">
          <Tools tools={tools} />
          <div className="cmp-history">
            <TipsButton />
            <button
              type="button"
              className="cmp-icon-btn"
              aria-label="Undo"
              data-tip="Undo"
              data-tip-kbd={MOD_KEYS.undo}
              data-tip-side="bottom"
              disabled={!canUndo(history)}
              onClick={() => dispatch({ type: "undo" })}
            >
              <UndoIcon size={16} />
            </button>
            <button
              type="button"
              className="cmp-icon-btn"
              aria-label="Redo"
              data-tip="Redo"
              data-tip-kbd={MOD_KEYS.redo}
              data-tip-side="bottom"
              disabled={!canRedo(history)}
              onClick={() => dispatch({ type: "redo" })}
            >
              <RedoIcon size={16} />
            </button>
            <button type="button" className="cmp-tool is-quiet" data-tip="Start a new design: empty, or from a template" data-tip-side="bottom" onClick={() => setStarting(true)}>
              <LayoutTemplateIcon size={16} />
              <span className="cmp-tool-label">New</span>
            </button>
          </div>
        </div>

        <div className="cmp-stage-wrap">
          <ComposerStage
            doc={doc}
            shown={side === "icons" ? iconPreview : null}
            pendingId={side === "icons" && iconPreview ? PREVIEW_ID : null}
            selectedId={selectedId}
            onSelect={setSelectedId}
            onPreview={(d) => dispatch({ type: "preview", doc: d })}
            onSettle={() => dispatch({ type: "settle" })}
            assets={assets}
            template={template?.images ?? null}
            folderLoading={folderLoading}
            parts={parts}
            view={viewOf}
            backdrop={view.backdrop}
            version={version}
            hint={doc.layers.length === 0 ? "An empty design is a see-through folder. Add a colour, words or a picture from the bar above." : null}
            onOpen={(layer) => {
              setSelectedId(layer.id);
              window.setTimeout(() => {
                if (layer.kind === "text") {
                  textRef.current?.focus();
                  textRef.current?.select();
                } else if (layer.kind === "emoji") document.querySelector<HTMLButtonElement>(".cmp-pick-btn.is-emoji")?.click();
              }, 30);
              if (layer.kind === "icon") openIcons(layer.id);
            }}
          />
        </div>

        <div className="cmp-bar" ref={barRef}>
          <button
            type="button"
            role="switch"
            aria-checked={doc.shape === "folder"}
            aria-label="folder skeleton"
            className="cmp-skeleton"
            data-tip={
              doc.shape === "folder"
                ? "On: your design is cut to the folder, with its tab and edges. Off: it's the whole icon, any shape you like."
                : "Off: your design is the whole icon, any shape you like. On: it's cut to the folder, with its tab and edges."
            }
            onClick={() => commit({ ...latestDoc.current, shape: doc.shape === "folder" ? "free" : "folder" })}
          >
            <span className={doc.shape === "folder" ? "switch is-on" : "switch"} aria-hidden="true">
              <span className="knob" />
            </span>
            Folder skeleton
          </button>
          {doc.shape === "folder" && (
            <LookSwitch value={doc.style} onChange={restyle} />
          )}
          <div className="cmp-backdrops" role="radiogroup" aria-label="what's behind the folder" ref={backdropsRef} hidden={!barRoom.backdrops}>
            {BACKDROPS.map((b) => (
              <button
                key={b.id}
                type="button"
                role="radio"
                aria-checked={view.backdrop === b.id}
                aria-label={b.label}
                data-tip={b.label}
                className={view.backdrop === b.id ? `cmp-backdrop is-${b.id} is-on` : `cmp-backdrop is-${b.id}`}
                onClick={() => setView((v) => ({ ...v, backdrop: b.id }))}
              />
            ))}
          </div>
          <div className="cmp-sizes" aria-label="the icon at its real sizes" data-tip={`How it looks in ${fileBrowserName()} at 64, 32 and 16 points`} ref={sizesRef} hidden={!barRoom.sizes}>
            {previews.map((src, i) => {
              const pt = PREVIEW_SIZES[i] / 2;
              return <img key={i} src={src} alt="" width={pt} height={pt} draggable={false} className="cmp-size" />;
            })}
          </div>
        </div>
      </section>

      <aside className={drag?.kind === "folder" ? "island cmp-side is-drop-target" : "island cmp-side"} aria-label="layers and settings" hidden={!active}>
        <span className="drop-glow" aria-hidden="true" />
        <div className="cmp-side-head">
          <input
            className="cmp-name"
            value={name}
            maxLength={MAX_NAME_CHARS}
            placeholder={suggested ?? "Name your skin"}
            aria-label="skin name"
            spellCheck={false}
            onChange={(e) => {
              setName(e.target.value);
              setNameTouched(true);
            }}
          />
          <p className="cmp-side-sub">{editing ? (dirty ? "Changed since it was saved" : "Saved in Yours") : "Not saved yet"}</p>
          <div className="cmp-side-tabs">
            <Segmented<"layers" | "icons">
              label="side panel"
              value={side}
              onChange={setSide}
              options={[
                { value: "layers", label: `Layers · ${doc.layers.length}` },
                { value: "icons", label: "Icons" },
              ]}
            />
          </div>
        </div>
        {iconsOpened && (
          <div className="cmp-icons-view" hidden={side !== "icons"}>
            <IconLibrary
              doc={doc}
              target={iconTarget}
              spot={iconSpot}
              onPreview={setIconPreview}
              look={iconTarget ? iconTarget.look : iconLook}
              onLook={chooseLook}
              onAdd={addIcon}
              onSwap={swapIcon}
              onDone={() => setSelectedId(null)}
              onError={(message) => toast(message, { tone: "danger" })}
            />
          </div>
        )}
        {side === "layers" && (
          <>
            <Panel
              title="Layers"
              badge={<span className="count">{doc.layers.length}</span>}
              open={panels.layers}
              onToggle={() => setPanels((p) => ({ ...p, layers: !p.layers }))}
              className={bothOpen ? "is-layers is-sized" : "is-layers"}
              style={bothOpen ? { flexBasis: panels.height } : undefined}
              panelRef={layersPanel}
            >
              <div className="cmp-layers-scroll">
                <ComposerLayers
                  doc={doc}
                  selectedId={selectedId}
                  onSelect={setSelectedId}
                  onToggle={(id, field) => commit(mapLayer(doc, id, (l) => ({ ...l, [field]: !l[field] }) as Layer))}
                  onRename={(id, value) => commit(mapLayer(doc, id, (l) => ({ ...l, name: value || undefined }) as Layer))}
                  onMove={(id, to) => commit(moveLayer(doc, id, to))}
                  onDelete={removeById}
                />
              </div>
            </Panel>
            {bothOpen && (
              <Resizer
                measure={measureLayers}
                onHeight={(height) => setPanels((p) => ({ ...p, height }))}
                onReset={() => setPanels((p) => ({ ...p, height: LAYERS_HEIGHT }))}
              />
            )}
            <Panel
              title="Attributes"
              actions={
                selected ? (
                  <>
                    <IconButton label="Bring forward" onClick={() => commit(bringForward(doc, selected.id))} disabled={index >= doc.layers.length - 1}>
                      <ChevronUpIcon size={15} />
                    </IconButton>
                    <IconButton label="Send backward" onClick={() => commit(sendBackward(doc, selected.id))} disabled={index <= 0}>
                      <ChevronDownIcon size={15} />
                    </IconButton>
                    <IconButton label="Duplicate" onClick={duplicate}>
                      <CopyIcon size={14} />
                    </IconButton>
                    <IconButton label="Delete" onClick={remove} className="is-danger">
                      <TrashIcon size={14} />
                    </IconButton>
                  </>
                ) : undefined
              }
              open={panels.settings}
              onToggle={() => setPanels((p) => ({ ...p, settings: !p.settings }))}
              className="is-settings"
              panelRef={settingsPanel}
            >
              <div className="cmp-side-scroll" key={selectedId ?? "design"}>
                <ComposerInspector
                  layer={selected}
                  onPatch={patch}
                  parts={parts}
                  used={used}
                  textRef={textRef}
                  onReplaceImage={(anchor) => setReplaceAnchor(anchor)}
                  onReplaceIcon={() => openIcons(selected?.id ?? null)}
                  index={index}
                  size={selected && isPlaced(selected) ? boxOf(selected, assets) : null}
                />
              </div>
            </Panel>
          </>
        )}
        <div className="cmp-save">
          <button type="button" className={folder ? "cmp-target" : "cmp-target is-empty"} onClick={onChooseFolder} disabled={busy} data-tip={folder ? "Choose another folder to apply it to" : "Choose the folder to apply it to"}>
            {folder && folderIcon ? <img src={folderIcon} alt="" draggable={false} /> : <FolderIcon size={18} />}
            <span className="cmp-target-text">
              <span className="cmp-target-label">{folder ? "Apply to" : "No folder chosen"}</span>
              <span className="cmp-target-name">{folder ? folder.name : "Choose a folder…"}</span>
            </span>
          </button>
          {folder && subfolders && subfolders.count > 0 && (
            <button
              type="button"
              role="switch"
              aria-checked={includeSubfolders}
              className={includeSubfolders ? "cmp-subfolders is-on" : "cmp-subfolders"}
              disabled={busy || subfolders.more}
              onClick={() => onIncludeSubfolders(!includeSubfolders)}
            >
              <span className="cmp-subfolders-text">
                {subfolders.more ? tooMany("subfolders") : `Include ${folderCount(subfolders.count)} inside`}
              </span>
              <span className={includeSubfolders ? "switch is-on" : "switch"} aria-hidden="true">
                <span className="knob" />
              </span>
            </button>
          )}
          {applying && progress ? (
            <div className="cmp-save-row" role="status" aria-live="polite">
              <button type="button" className="btn btn-secondary" disabled={stopping} onClick={onStop}>
                {stopping ? <LoaderIcon size={15} /> : null}
                {stopping ? "Stopping" : "Stop"}
              </button>
              <button type="button" className="btn btn-primary cmp-progress-btn" disabled aria-busy="true" style={{ "--done": `${progress.total ? (progress.done / progress.total) * 100 : 0}%` } as CSSProperties}>
                <LoaderIcon size={15} />
                Applying {progress.done.toLocaleString("en-US")} of {progress.total.toLocaleString("en-US")}
              </button>
            </div>
          ) : (
            <div className={secondary ? "cmp-save-row" : "cmp-save-row is-single"}>
              {secondary && (
                <button type="button" className="btn btn-secondary" disabled={busy} onClick={() => void save(secondary.mode)}>
                  {saving === secondary.mode ? <LoaderIcon size={15} /> : null}
                  {secondary.label}
                </button>
              )}
              <button type="button" className="btn btn-primary" disabled={primaryDisabled} onClick={() => void save(primary.mode)} aria-busy={saving === primary.mode || (primary.mode === "apply" && applying)}>
                {saving === primary.mode || (primary.mode === "apply" && applying) ? <LoaderIcon size={15} /> : null}
                {primary.label}
              </button>
            </div>
          )}
        </div>
      </aside>

      {replaceAnchor && selected?.kind === "image" && (
        <Popover anchor={replaceAnchor} onClose={() => setReplaceAnchor(null)} width={300} label="Replace the picture" align="end">
          <PictureMenu
            skins={skins}
            onFile={() => {
              setReplaceAnchor(null);
              void choosePicture().then((img) => img && replacePicture(selected.id, img));
            }}
            onSkin={(s) => {
              setReplaceAnchor(null);
              void addSkinPicture(s, selected.id);
            }}
          />
        </Popover>
      )}
      {active && starting && (
        <NewDesign
          parts={parts}
          template={template?.images ?? null}
          assets={assets}
          version={version}
          dirty={dirty && doc.layers.length > 0}
          saving={saving !== null}
          onStart={(choice) => void start(choice)}
          onSaveFirst={() => void save("save")}
          onClose={() => setStarting(false)}
        />
      )}
      {confirm && (
        <Confirm
          title={confirm.title}
          text={confirm.text}
          action={confirm.action}
          onCancel={() => setConfirm(null)}
          onConfirm={() => {
            const run = confirm.run;
            setConfirm(null);
            run();
          }}
        />
      )}
    </>
  );
}
