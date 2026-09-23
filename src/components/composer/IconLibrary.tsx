import { memo, useCallback, useDeferredValue, useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { centreOf, ICON_LOOKS, iconName, makeIcon, type Doc, type IconDrawing, type IconLayer, type IconLook, type Parts } from "../../composer/doc";
import { ICON_PACKS, type IconPackInfo } from "../../composer/icons/catalog";
import { drawingOf, searchIcons, type IconDef, type IconPack } from "../../composer/icons/index";
import { availablePacks, BUILTIN_PACK, downloadPack, loadPack, packInfo, removePack, type LoadedPack } from "../../composer/icons/load";
import { errorMessage } from "../../lib/tauri";
import { localOs } from "../../lib/platform";
import { VirtualGrid, type VirtualGridHandle } from "../VirtualGrid";
import { ChevronDownIcon, TickIcon, TrashIcon } from "../icons/composer";
import { DownloadIcon } from "../icons/download";
import { LoaderIcon } from "../icons/loader";
import { SearchIcon } from "../icons/search";
import { Segmented } from "./controls";
import { PackLogo } from "./PackLogo";
import { Popover } from "./Popover";

const PACK_KEY = "folderskin.composer.iconPack";
/** The narrowest an icon cell gets; the grid fits as many as the side has room for. */
const CELL = 46;

function rememberedPack(): string {
  try {
    const v = localStorage.getItem(PACK_KEY);
    return v && ICON_PACKS.some((p) => p.id === v) ? v : BUILTIN_PACK;
  } catch {
    return BUILTIN_PACK;
  }
}

function rememberPack(id: string) {
  try {
    localStorage.setItem(PACK_KEY, id);
  } catch {
    // Only a preference.
  }
}

const size = (bytes: number) => (bytes >= 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : `${Math.round(bytes / 1024)} KB`);
/** "2,112 icons", or "327 logos" for a pack of brands. */
const counted = (p: IconPackInfo) => `${p.count.toLocaleString("en-US")} ${p.brands ? "logos" : "icons"}`;

/** An icon drawn with SVG, for the grid: cheap enough for the few hundred on screen at once. */
export const IconGlyph = memo(function IconGlyph({ pack, icon, size }: { pack: IconPack; icon: IconDef; size: number }) {
  return (
    <svg viewBox={`0 0 ${pack.viewBox} ${pack.viewBox}`} width={size} height={size} aria-hidden="true">
      {icon.d.map((d, i) =>
        pack.style === "fill" || icon.f?.includes(i) ? (
          <path key={i} d={d} fill="currentColor" fillRule={icon.r ? "evenodd" : "nonzero"} />
        ) : (
          <path key={i} d={d} fill="none" stroke="currentColor" strokeWidth={pack.strokeWidth} strokeLinecap="round" strokeLinejoin="round" />
        ),
      )}
    </svg>
  );
});

/** The packs, each with its logo and how many icons it has: choose one, download one, or remove one. */
function PackList({
  current,
  available,
  progress,
  onChoose,
  onDownload,
  onRemove,
}: {
  current: string;
  available: Set<string>;
  progress: Record<string, number>;
  onChoose: (id: string) => void;
  onDownload: (info: IconPackInfo) => void;
  onRemove: (info: IconPackInfo) => void;
}) {
  return (
    <div className="icon-packs" role="list" aria-label="icon packs">
      {ICON_PACKS.map((p) => {
        const have = available.has(p.id);
        const got = progress[p.id];
        return (
          <div key={p.id} role="listitem" className={p.id === current ? "icon-pack is-current" : "icon-pack"}>
            <button
              type="button"
              className="icon-pack-main"
              disabled={!have}
              aria-current={p.id === current}
              data-tip={have ? undefined : `Download ${p.name} to use it`}
              onClick={() => onChoose(p.id)}
            >
              <PackLogo id={p.id} name={p.name} size={18} />
              <span className="icon-pack-text">
                <span className="icon-pack-name">{p.name}</span>
                <span className="icon-pack-meta">{counted(p)}</span>
              </span>
              {p.id === current && <TickIcon size={15} />}
            </button>
            {!have && (
              <button
                type="button"
                className="icon-pack-get"
                disabled={got !== undefined}
                aria-label={`download ${p.name}`}
                data-tip={got !== undefined ? undefined : `Download ${p.name}, ${size(p.bytes)}`}
                onClick={() => onDownload(p)}
              >
                {got !== undefined ? (
                  <>
                    <LoaderIcon size={13} />
                    {Math.round(got * 100)}%
                  </>
                ) : (
                  <>
                    <DownloadIcon size={13} />
                    {size(p.bytes)}
                  </>
                )}
              </button>
            )}
            {have && !p.builtin && (
              <button type="button" className="icon-pack-remove" aria-label={`remove ${p.name}`} data-tip="Remove from this computer" onClick={() => onRemove(p)}>
                <TrashIcon size={13} />
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}

/**
 * The icon library in the composer's side island: every icon of the chosen pack, searchable,
 * drawn only as far as the grid is scrolled. The icon under the pointer (or the keyboard) is shown
 * on the canvas itself, live, before anything changes. With an icon on the design the library works
 * on it (the selected one, or else the top one): a click swaps it for the one clicked, so trying
 * icons never piles them up, ⌥-click adds another instead, and the look switch changes it in
 * place. Lucide comes with the app; the other packs are a download away.
 */
export function IconLibrary({
  doc,
  parts,
  target,
  look,
  onLook,
  onPick,
  onPreview,
  onError,
}: {
  doc: Doc;
  parts: Parts;
  /** The icon a pick replaces (the selected one, or the design's top one); null adds a new one. */
  target: IconLayer | null;
  /** The selected icon's look, or the next new one's. */
  look: IconLook;
  onLook: (look: IconLook) => void;
  /** An icon was picked: `asNew` adds it even with an icon selected. */
  onPick: (drawing: IconDrawing, asNew: boolean) => void;
  /** The design as a click would leave it, for the canvas to show; null when nothing is pointed at. */
  onPreview: (doc: Doc | null) => void;
  onError: (message: string) => void;
}) {
  const [packId, setPackId] = useState(rememberedPack);
  const [available, setAvailable] = useState<Set<string>>(() => new Set([BUILTIN_PACK]));
  const [loaded, setLoaded] = useState<LoadedPack | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const deferred = useDeferredValue(query);
  const [hover, setHover] = useState<IconDef | null>(null);
  const [active, setActive] = useState(0);
  /** The arrow keys are moving through the grid, so the icon they're on is the one shown. */
  const [keyed, setKeyed] = useState(false);
  const [packsAnchor, setPacksAnchor] = useState<HTMLElement | null>(null);
  const [progress, setProgress] = useState<Record<string, number>>({});
  const grid = useRef<VirtualGridHandle>(null);

  useEffect(() => {
    void availablePacks().then(setAvailable);
  }, []);

  // A remembered pack that isn't on this computer any more falls back to the built-in one.
  const usable = available.has(packId) ? packId : BUILTIN_PACK;
  useEffect(() => {
    let live = true;
    setFailed(null);
    loadPack(usable)
      .then((l) => live && setLoaded(l))
      .catch((e) => live && setFailed(errorMessage(e)));
    return () => {
      live = false;
    };
  }, [usable]);

  const pack = loaded?.pack.id === usable ? loaded.pack : null;
  const results = useMemo(() => (loaded && pack ? searchIcons(pack, loaded.index, deferred) : []), [loaded, pack, deferred]);
  // New results: the first is the one in hand, and an icon that was under the pointer before is
  // gone from the grid even if the pointer hasn't moved (so no pointerleave told us).
  useEffect(() => {
    setActive(0);
    setHover(null);
  }, [results]);

  const choosePack = (id: string) => {
    setPackId(id);
    rememberPack(id);
    setPacksAnchor(null);
    setHover(null);
  };

  const download = async (info: IconPackInfo) => {
    setProgress((p) => ({ ...p, [info.id]: 0 }));
    try {
      await downloadPack(info, (p) => setProgress((all) => ({ ...all, [info.id]: p.total ? p.done / p.total : 0 })));
      setAvailable(await availablePacks());
      choosePack(info.id);
    } catch (e) {
      onError(`Couldn't download ${info.name}: ${errorMessage(e)}`);
    } finally {
      setProgress(({ [info.id]: _, ...rest }) => rest);
    }
  };

  const remove = async (info: IconPackInfo) => {
    try {
      await removePack(info.id);
      setAvailable(await availablePacks());
      if (packId === info.id) choosePack(BUILTIN_PACK);
    } catch (e) {
      onError(`Couldn't remove ${info.name}: ${errorMessage(e)}`);
    }
  };

  const pick = useCallback(
    (icon: IconDef, asNew: boolean) => {
      if (pack) onPick(drawingOf(pack, icon), asNew);
    },
    [pack, onPick],
  );

  // The design as a click would leave it: the icon pointed at in the target's place, or added to the
  // front's middle. Never both, so the canvas shows exactly what a click does.
  const front = centreOf(parts.front);
  const shown = hover ?? (keyed ? (results[active] ?? null) : null);
  const preview = useMemo<Doc | null>(() => {
    if (!pack || !shown) return null;
    const drawing = drawingOf(pack, shown);
    if (target) {
      return {
        ...doc,
        layers: doc.layers.map((l) =>
          l.id === target.id && l.kind === "icon"
            ? { ...l, ...drawing, filled: drawing.filled ?? [], evenOdd: drawing.evenOdd === true, look: l.look === "original" && !drawing.brand ? "flat" : l.look }
            : l,
        ),
      };
    }
    return { ...doc, layers: [...doc.layers, { ...makeIcon(drawing, front.x, front.y, look), id: "preview" }] };
  }, [doc, pack, shown, target, look, front.x, front.y]);
  useEffect(() => onPreview(preview), [preview, onPreview]);
  useEffect(() => () => onPreview(null), [onPreview]);

  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (results.length === 0) return;
    const cols = grid.current?.columns() ?? 1;
    const step = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: cols, ArrowUp: -cols }[e.key];
    if (step !== undefined) {
      e.preventDefault();
      const next = Math.max(0, Math.min(results.length - 1, active + step));
      setActive(next);
      setHover(null);
      setKeyed(true);
      grid.current?.scrollToIndex(next);
      requestAnimationFrame(() => grid.current?.element()?.querySelector<HTMLElement>(`[data-index="${next}"] button`)?.focus());
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      pick(results[active], e.altKey);
    }
  };

  const info = packInfo(usable);
  const looks = ICON_LOOKS.filter((l) => l.id !== "original" || target?.brand || pack?.brands);
  // A line under the grid names the icon pointed at (the cells have no tooltips of their own) and
  // says what a click does; the canvas shows it.
  const same = target !== null && shown !== null && target.pack === pack?.id && target.icon === shown.n;
  const alt = localOs() === "macos" ? "⌥" : "Alt";
  const status = !shown
    ? target
      ? `Point at an icon to try it in place of ${iconName(target.icon)}.`
      : "Point at an icon to try it on your folder."
    : same
      ? "It's on your folder now."
      : target
        ? `Click to swap it for ${iconName(target.icon)}, ${alt}-click to add another.`
        : "Click to add it to the front.";
  return (
    <div className="icon-library">
      <div className="icon-controls">
        <button
          type="button"
          className="cmp-pick-btn icon-pack-btn"
          aria-haspopup="dialog"
          aria-expanded={packsAnchor !== null}
          aria-label={`icon pack: ${info?.name ?? usable}`}
          data-tip="Icon packs"
          onClick={(e) => setPacksAnchor(packsAnchor ? null : e.currentTarget)}
        >
          <PackLogo id={usable} name={info?.name ?? usable} size={15} />
          <span>{info?.name ?? usable}</span>
          <ChevronDownIcon size={14} />
        </button>
        <Segmented<IconLook>
          label={target ? "how the selected icon looks" : "how new icons look"}
          small
          value={look}
          onChange={onLook}
          options={looks.map((l) => ({ value: l.id, label: l.label, title: target ? `${l.label}: changes ${iconName(target.icon)} on the folder` : `${l.label}: how the next icon looks` }))}
        />
      </div>
      <label className="search icon-search">
        <SearchIcon size={15} />
        <input
          type="search"
          value={query}
          placeholder={pack ? `Search ${pack.icons.length.toLocaleString("en-US")} ${info?.brands ? "logos" : "icons"}` : "Search icons"}
          aria-label="search icons"
          spellCheck={false}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown" && results.length > 0) {
              e.preventDefault();
              grid.current?.element()?.querySelector<HTMLElement>(`[data-index="${active}"] button`)?.focus();
            }
          }}
        />
      </label>
      {failed ? (
        <p className="icon-empty">{failed}</p>
      ) : !pack ? (
        <p className="icon-empty">
          <LoaderIcon size={15} /> Opening {info?.name ?? "the icons"}…
        </p>
      ) : (
        <div className="icon-grid-wrap" onKeyDown={onKey} onPointerMove={() => keyed && setKeyed(false)}>
          <VirtualGrid
            ref={grid}
            items={results}
            minCell={CELL}
            gap={4}
            rowHeight={(w) => w}
            overscan={3}
            getKey={(icon) => icon.n}
            className="icon-grid"
            role="grid"
            aria-label={`${pack.name} icons`}
            empty={<p className="icon-empty">No icons in {pack.name} match “{deferred.trim()}”. Try another word, or another pack.</p>}
            renderItem={(icon, i) => (
              <button
                type="button"
                className={i === active ? "icon-cell is-active" : "icon-cell"}
                tabIndex={i === active ? 0 : -1}
                aria-label={iconName(icon.n)}
                onPointerEnter={() => setHover(icon)}
                onPointerLeave={() => setHover((h) => (h === icon ? null : h))}
                onFocus={() => setActive(i)}
                onClick={(e) => {
                  setActive(i);
                  pick(icon, e.altKey);
                }}
              >
                <IconGlyph pack={pack} icon={icon} size={22} />
              </button>
            )}
          />
        </div>
      )}
      <p className="icon-status" aria-live="polite">
        {shown && <strong>{iconName(shown.n)}</strong>}
        <span>{status}</span>
      </p>
      {packsAnchor && (
        <Popover anchor={packsAnchor} onClose={() => setPacksAnchor(null)} width={280} label="Icon packs">
          <PackList current={usable} available={available} progress={progress} onChoose={choosePack} onDownload={(p) => void download(p)} onRemove={(p) => void remove(p)} />
        </Popover>
      )}
    </div>
  );
}
