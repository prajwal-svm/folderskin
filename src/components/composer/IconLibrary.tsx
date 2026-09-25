import "../../i18n/composer";
import { t, useLocale } from "../../i18n";
import { formatNumber } from "../../i18n/format";
import { memo, useCallback, useDeferredValue, useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { ICON_LOOKS, iconName, makeIcon, type Doc, type IconDrawing, type IconLayer, type IconLook } from "../../composer/doc";
import { ICON_PACKS, type IconPackInfo } from "../../composer/icons/catalog";
import { drawingOf, searchIcons, type IconDef, type IconPack } from "../../composer/icons/index";
import { availablePacks, BUILTIN_PACK, downloadPack, loadPack, packInfo, removePack, type LoadedPack } from "../../composer/icons/load";
import { errorMessage } from "../../lib/tauri";
import { VirtualGrid, type VirtualGridHandle } from "../VirtualGrid";
import { ChevronDownIcon, PlusIcon, TickIcon, TrashIcon } from "../icons/composer";
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

/** A download's size, as the pack list gives it: "3.2 MB", "640 KB" (in 1,024s, as it always has). */
const size = (bytes: number) =>
  bytes >= 1024 * 1024
    ? t("common.size.mb", { value: formatNumber(bytes / 1024 / 1024, { minimumFractionDigits: 1, maximumFractionDigits: 1 }) })
    : t("common.size.kb", { value: formatNumber(Math.round(bytes / 1024)) });
/** "2,112 icons", or "327 logos" for a pack of brands. */
const counted = (p: IconPackInfo) => (p.brands ? t("composer.icons.logos", { count: p.count }) : t("composer.icons.icons", { count: p.count }));

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

/** The id of the icon being tried on the canvas, so the canvas can outline it as not added yet. */
export const PREVIEW_ID = "preview";

/** An icon layer drawn small, for the bar under the grid. */
function LayerGlyph({ layer }: { layer: IconLayer }) {
  return (
    <svg viewBox={`0 0 ${layer.viewBox} ${layer.viewBox}`} width={18} height={18}>
      {layer.paths.map((d, i) =>
        layer.style === "fill" || layer.filled.includes(i) ? (
          <path key={i} d={d} fill="currentColor" fillRule={layer.evenOdd ? "evenodd" : "nonzero"} />
        ) : (
          <path key={i} d={d} fill="none" stroke="currentColor" strokeWidth={layer.strokeWidth} strokeLinecap="round" strokeLinejoin="round" />
        ),
      )}
    </svg>
  );
}

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
    <div className="icon-packs" role="list" aria-label={t("composer.icons.packsLabel")}>
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
              data-tip={have ? undefined : t("composer.icons.downloadToUse", { name: p.name })}
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
                aria-label={t("composer.icons.downloadLabel", { name: p.name })}
                data-tip={got !== undefined ? undefined : t("composer.icons.downloadTip", { name: p.name, size: size(p.bytes) })}
                onClick={() => onDownload(p)}
              >
                {got !== undefined ? (
                  <>
                    <LoaderIcon size={13} />
                    {formatNumber(got, { style: "percent", maximumFractionDigits: 0 })}
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
              <button type="button" className="icon-pack-remove" aria-label={t("composer.icons.removeLabel", { name: p.name })} data-tip={t("composer.icons.removeTip")} onClick={() => onRemove(p)}>
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
 * drawn only as far as the grid is scrolled. It works one of two ways, by what's selected:
 *
 * - Nothing selected, it adds. A click puts the icon on the canvas to try, outlined as not added
 *   yet; another click tries another in its place; Add to canvas (or a double-click, or Enter)
 *   keeps it, in a free spot, and the next click starts another. Adding one never changes one
 *   that's there.
 * - An icon selected, it swaps: a click puts the icon clicked in its place, undoably, and Done
 *   goes back to adding.
 *
 * The icon under the pointer is shown on the canvas live either way, and the look switch sets
 * the selected icon's look or the next one's. Lucide comes with the app; the other packs are a
 * download away.
 */
export function IconLibrary({
  doc,
  target,
  spot,
  look,
  onLook,
  onAdd,
  onSwap,
  onDone,
  onPreview,
  onError,
}: {
  doc: Doc;
  /** The selected icon, which a click swaps; null while adding. */
  target: IconLayer | null;
  /** Where the next icon added goes, how big and in what colour: a free spot on the front. */
  spot: { x: number; y: number; size: number; color: string };
  /** The selected icon's look, or the next new one's. */
  look: IconLook;
  onLook: (look: IconLook) => void;
  onAdd: (drawing: IconDrawing) => void;
  onSwap: (drawing: IconDrawing) => void;
  /** Stops swapping the selected icon, to add others. */
  onDone: () => void;
  /** The design as the canvas should show it (the icon being tried in it); null for the design as it is. */
  onPreview: (doc: Doc | null) => void;
  onError: (message: string) => void;
}) {
  useLocale();
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
  /** The icon being tried, while adding: on the canvas, outlined, until it's added or another is tried. */
  const [candidate, setCandidate] = useState<IconDef | null>(null);
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
    setCandidate(null);
  };

  // Selecting an icon on the canvas switches to swapping it; whatever was being tried goes.
  const targetId = target?.id ?? null;
  useEffect(() => setCandidate(null), [targetId]);

  const download = async (info: IconPackInfo) => {
    setProgress((p) => ({ ...p, [info.id]: 0 }));
    try {
      await downloadPack(info, (p) => setProgress((all) => ({ ...all, [info.id]: p.total ? p.done / p.total : 0 })));
      setAvailable(await availablePacks());
      choosePack(info.id);
    } catch (e) {
      onError(t("composer.icons.downloadFailed", { name: info.name, reason: errorMessage(e) }));
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
      onError(t("composer.icons.removeFailed", { name: info.name, reason: errorMessage(e) }));
    }
  };

  /** Keeps `icon`: adds it (the one being tried, or straight away) or puts it in the selected one's place. */
  const keep = useCallback(
    (icon: IconDef) => {
      if (!pack) return;
      if (target) onSwap(drawingOf(pack, icon));
      else {
        onAdd(drawingOf(pack, icon));
        setCandidate(null);
      }
    },
    [pack, target, onAdd, onSwap],
  );

  const click = (icon: IconDef) => {
    if (target) keep(icon);
    else setCandidate(icon);
  };

  // What the canvas shows: the icon under the pointer (or the keyboard), else the one being tried,
  // in the selected icon's place or in the free spot. Always exactly what keeping it would do.
  const shown = hover ?? (keyed ? (results[active] ?? null) : null) ?? (target ? null : candidate);
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
    return { ...doc, layers: [...doc.layers, { ...makeIcon(drawing, spot.x, spot.y, look, spot.color, spot.size), id: PREVIEW_ID }] };
  }, [doc, pack, shown, target, look, spot.x, spot.y, spot.size, spot.color]);
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
    } else if (e.key === "Enter") {
      e.preventDefault();
      keep(results[active]);
    } else if (e.key === " ") {
      e.preventDefault();
      click(results[active]);
    } else if (e.key === "Escape" && candidate) {
      e.stopPropagation();
      setCandidate(null);
    }
  };

  const info = packInfo(usable);
  // Original keeps the colours a pack gives its icons (a brand's), so it's offered for a selected
  // logo, or while adding from a pack whose icons have colours. Where it isn't offered, a new icon
  // comes out flat, and the switch says so.
  const coloured = useMemo(() => pack?.icons.some((i) => i.c !== undefined) ?? false, [pack]);
  const looks = ICON_LOOKS.filter((l) => l.id !== "original" || (target ? target.brand : coloured));
  const shownLook = looks.some((l) => l.id === look) ? look : "flat";
  // The bar under the grid: while swapping, which icon and Done; while adding, the icon being tried
  // and Add to canvas, or the icon pointed at (the cells have no tooltips of their own).
  const named = candidate ?? hover;
  return (
    <div className="icon-library">
      <div className="icon-controls">
        <button
          type="button"
          className="cmp-pick-btn icon-pack-btn"
          aria-haspopup="dialog"
          aria-expanded={packsAnchor !== null}
          aria-label={t("composer.icons.packLabel", { name: info?.name ?? usable })}
          data-tip={t("composer.icons.packs")}
          onClick={(e) => setPacksAnchor(packsAnchor ? null : e.currentTarget)}
        >
          <PackLogo id={usable} name={info?.name ?? usable} size={15} />
          <span>{info?.name ?? usable}</span>
          <ChevronDownIcon size={14} />
        </button>
        <Segmented<IconLook>
          label={target ? t("composer.icons.lookSelected") : t("composer.icons.lookNew")}
          small
          value={shownLook}
          onChange={onLook}
          options={looks.map((l) => {
            const label = t(`composer.iconLooks.${l.id}`);
            return { value: l.id, label, title: target ? t("composer.icons.lookChanges", { look: label, icon: iconName(target.icon) }) : t("composer.icons.lookNext", { look: label }) };
          })}
        />
      </div>
      <label className="search icon-search">
        <SearchIcon size={15} />
        <input
          type="search"
          value={query}
          placeholder={pack ? (info?.brands ? t("composer.icons.searchLogos", { count: pack.icons.length }) : t("composer.icons.searchIcons", { count: pack.icons.length })) : t("composer.icons.search")}
          aria-label={t("composer.icons.searchLabel")}
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
          <LoaderIcon size={15} /> {info ? t("composer.icons.opening", { name: info.name }) : t("composer.icons.openingIcons")}
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
            aria-label={t("composer.icons.gridLabel", { name: pack.name })}
            empty={<p className="icon-empty">{t("composer.icons.noMatch", { name: pack.name, query: deferred.trim() })}</p>}
            renderItem={(icon, i) => (
              <button
                type="button"
                className={i === active ? "icon-cell is-active" : "icon-cell"}
                tabIndex={i === active ? 0 : -1}
                aria-label={iconName(icon.n)}
                onPointerEnter={() => setHover(icon)}
                onPointerLeave={() => setHover((h) => (h === icon ? null : h))}
                aria-pressed={!target && candidate === icon}
                onFocus={() => setActive(i)}
                onClick={() => {
                  setActive(i);
                  click(icon);
                }}
                onDoubleClick={() => {
                  if (!target) keep(icon);
                }}
              >
                <IconGlyph pack={pack} icon={icon} size={22} />
              </button>
            )}
          />
        </div>
      )}
      <div className="icon-status" aria-live="polite">
        {target ? (
          <>
            <span className="icon-status-glyph" aria-hidden="true">
              <LayerGlyph layer={target} />
            </span>
            <span className="icon-status-text">
              <strong>{iconName(target.icon)}</strong>
              <span>{t("composer.icons.swapHint")}</span>
            </span>
            <button type="button" className="btn btn-secondary btn-xs" data-tip={t("composer.icons.doneTip")} onClick={onDone}>
              {t("share.done")}
            </button>
          </>
        ) : named && pack ? (
          <>
            <span className="icon-status-glyph" aria-hidden="true">
              <IconGlyph pack={pack} icon={named} size={18} />
            </span>
            <span className="icon-status-text">
              <strong>{iconName(named.n)}</strong>
              <span>{candidate ? t("composer.icons.trying") : t("composer.icons.clickToTry")}</span>
            </span>
            {candidate && (
              <button type="button" className="btn btn-primary btn-xs" data-tip={t("composer.icons.addTip", { icon: iconName(candidate.n) })} data-tip-kbd="Enter" onClick={() => keep(candidate)}>
                <PlusIcon size={13} />
                {t("composer.icons.add")}
              </button>
            )}
          </>
        ) : (
          <span className="icon-status-text">
            <span>{t("composer.icons.hint")}</span>
          </span>
        )}
      </div>
      {packsAnchor && (
        <Popover anchor={packsAnchor} onClose={() => setPacksAnchor(null)} width={280} label={t("composer.icons.packs")}>
          <PackList current={usable} available={available} progress={progress} onChoose={choosePack} onDownload={(p) => void download(p)} onRemove={(p) => void remove(p)} />
        </Popover>
      )}
    </div>
  );
}
