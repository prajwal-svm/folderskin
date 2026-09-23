import { memo, useCallback, useDeferredValue, useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import type { Assets } from "../../composer/assets";
import type { TemplateImages } from "../../composer/composite";
import { centreOf, ICON_LOOKS, iconName, makeIcon, type Doc, type IconDrawing, type IconLayer, type IconLook, type Parts } from "../../composer/doc";
import { ICON_PACKS, type IconPackInfo } from "../../composer/icons/catalog";
import { drawingOf, searchIcons, type IconDef, type IconPack } from "../../composer/icons/index";
import { availablePacks, BUILTIN_PACK, downloadPack, loadPack, packInfo, removePack, type LoadedPack } from "../../composer/icons/load";
import { errorMessage } from "../../lib/tauri";
import { VirtualGrid, type VirtualGridHandle } from "../VirtualGrid";
import { ChevronDownIcon, XIcon } from "../icons/composer";
import { LoaderIcon } from "../icons/loader";
import { SearchIcon } from "../icons/search";
import { Segmented } from "./controls";
import { Popover } from "./Popover";
import { DesignThumb } from "./NewDesign";

const PACK_KEY = "folderskin.composer.iconPack";
const LOOK_KEY = "folderskin.composer.iconLook";
/** The narrowest an icon cell gets; the grid fits as many as the side has room for. */
const CELL = 46;

function remembered<T extends string>(key: string, ok: readonly T[], fallback: T): T {
  try {
    const v = localStorage.getItem(key);
    return v && (ok as readonly string[]).includes(v) ? (v as T) : fallback;
  } catch {
    return fallback;
  }
}

function remember(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Only a preference.
  }
}

const kb = (bytes: number) => (bytes >= 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : `${Math.round(bytes / 1024)} KB`);

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

/** The packs, to switch between, download or remove. */
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
            <button type="button" className="icon-pack-main" disabled={!have} onClick={() => onChoose(p.id)} aria-current={p.id === current}>
              <span className="icon-pack-name">
                {p.name}
                {p.brands && <span className="chip chip-text icon-pack-tag">Logos</span>}
              </span>
              <span className="icon-pack-meta">
                {p.count.toLocaleString("en-US")} icons · {p.license}
                {p.builtin ? " · built in" : ""}
              </span>
            </button>
            {!have && (
              <button type="button" className="btn btn-secondary icon-pack-get" disabled={got !== undefined} onClick={() => onDownload(p)} aria-label={`download ${p.name}`}>
                {got !== undefined ? (
                  <>
                    <LoaderIcon size={14} />
                    {Math.round(got * 100)}%
                  </>
                ) : (
                  kb(p.bytes)
                )}
              </button>
            )}
            {have && !p.builtin && (
              <button type="button" className="link-btn icon-pack-remove" onClick={() => onRemove(p)} aria-label={`remove ${p.name}`}>
                Remove
              </button>
            )}
          </div>
        );
      })}
      <p className="icon-packs-note">Downloaded packs are checked against the copy this version of FolderSkin was built with, and kept on this computer.</p>
    </div>
  );
}

/**
 * The icon library in the composer's side island: every icon of the chosen pack, searchable,
 * drawn only as far as the grid is scrolled, and a preview of the design with the icon under the
 * pointer pressed into it, before anything is added. Lucide comes with the app; the other packs
 * are a download away.
 */
export function IconLibrary({
  doc,
  parts,
  template,
  assets,
  version,
  replacing,
  onPick,
  onCancelReplace,
  onError,
}: {
  doc: Doc;
  parts: Parts;
  template: TemplateImages | null;
  assets: Assets;
  version: number;
  /** The icon layer being swapped, when the library opened from its Replace button. */
  replacing: IconLayer | null;
  onPick: (drawing: IconDrawing, look: IconLook) => void;
  onCancelReplace: () => void;
  onError: (message: string) => void;
}) {
  const [packId, setPackId] = useState(() => remembered(PACK_KEY, ICON_PACKS.map((p) => p.id), BUILTIN_PACK));
  const [available, setAvailable] = useState<Set<string>>(() => new Set([BUILTIN_PACK]));
  const [loaded, setLoaded] = useState<LoadedPack | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const deferred = useDeferredValue(query);
  const [look, setLook] = useState<IconLook>(() => remembered(LOOK_KEY, ["emboss", "flat"] as const, "emboss"));
  const [hover, setHover] = useState<IconDef | null>(null);
  const [active, setActive] = useState(0);
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
  useEffect(() => setActive(0), [results]);

  const choosePack = (id: string) => {
    setPackId(id);
    remember(PACK_KEY, id);
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
    (icon: IconDef) => {
      if (pack) onPick(drawingOf(pack, icon), look);
    },
    [pack, look, onPick],
  );

  // The design with the icon under the pointer in it, as it would be once added.
  const front = centreOf(parts.front);
  const shown = hover ?? results[active] ?? null;
  const preview = useMemo<Doc>(() => {
    if (!pack || !shown) return doc;
    const drawing = drawingOf(pack, shown);
    if (replacing) {
      return { ...doc, layers: doc.layers.map((l) => (l.id === replacing.id ? { ...replacing, ...drawing, filled: drawing.filled ?? [], evenOdd: drawing.evenOdd === true } : l)) };
    }
    return { ...doc, layers: [...doc.layers, { ...makeIcon(drawing, front.x, front.y, look), id: "preview" }] };
  }, [doc, pack, shown, replacing, look, front.x, front.y]);

  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (results.length === 0) return;
    const cols = grid.current?.columns() ?? 1;
    const step = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: cols, ArrowUp: -cols }[e.key];
    if (step !== undefined) {
      e.preventDefault();
      const next = Math.max(0, Math.min(results.length - 1, active + step));
      setActive(next);
      setHover(null);
      grid.current?.scrollToIndex(next);
      requestAnimationFrame(() => grid.current?.element()?.querySelector<HTMLElement>(`[data-index="${next}"] button`)?.focus());
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      pick(results[active]);
    }
  };

  const info = packInfo(usable);
  return (
    <div className="icon-library">
      {replacing && (
        <div className="icon-replacing" role="status">
          <span>
            Replacing <strong>{iconName(replacing.icon)}</strong>. Pick another icon.
          </span>
          <button type="button" className="icon-btn" aria-label="stop replacing" title="Keep it as it is" onClick={onCancelReplace}>
            <XIcon size={14} />
          </button>
        </div>
      )}
      <div className="icon-preview">
        <DesignThumb doc={preview} template={template} assets={assets} size={124} version={version} />
        <div className="icon-preview-text">
          <span className="icon-preview-name">{shown ? iconName(shown.n) : "Pick an icon"}</span>
          <span className="icon-preview-sub">{shown ? `${pack?.name ?? ""} · ${replacing ? "replaces the one there" : "click to add it to the front"}` : "Point at one to see it on your folder."}</span>
        </div>
      </div>
      <div className="icon-controls">
        <button
          type="button"
          className="cmp-select icon-pack-btn"
          aria-haspopup="dialog"
          aria-expanded={packsAnchor !== null}
          aria-label={`icon pack: ${info?.name ?? usable}`}
          onClick={(e) => setPacksAnchor(packsAnchor ? null : e.currentTarget)}
        >
          <span className="icon-pack-btn-name">{info?.name ?? usable}</span>
          <ChevronDownIcon size={14} />
        </button>
        {!replacing && (
          <Segmented<IconLook>
            label="how new icons look"
            small
            value={look}
            onChange={(l) => {
              setLook(l);
              remember(LOOK_KEY, l);
            }}
            options={ICON_LOOKS.filter((l) => l.id !== "original").map((l) => ({ value: l.id, label: l.label }))}
          />
        )}
      </div>
      <label className="search icon-search">
        <SearchIcon size={15} />
        <input
          type="search"
          value={query}
          placeholder={pack ? `Search ${pack.icons.length.toLocaleString("en-US")} icons` : "Search icons"}
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
        <div className="icon-grid-wrap" onKeyDown={onKey}>
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
                title={iconName(icon.n)}
                onPointerEnter={() => setHover(icon)}
                onPointerLeave={() => setHover((h) => (h === icon ? null : h))}
                onFocus={() => setActive(i)}
                onClick={() => {
                  setActive(i);
                  pick(icon);
                }}
              >
                <IconGlyph pack={pack} icon={icon} size={22} />
              </button>
            )}
          />
        </div>
      )}
      <p className="icon-credit">
        {info && (
          <>
            {info.name} {info.version}, {info.license}.{" "}
          </>
        )}
        {info?.brands ? "These logos belong to their owners: fine on your own folders, but a skin that uses one can't be shared in Community." : "Free to use on anything, shared packs included."}
      </p>
      {packsAnchor && (
        <Popover anchor={packsAnchor} onClose={() => setPacksAnchor(null)} width={300} label="Icon packs">
          <PackList current={usable} available={available} progress={progress} onChoose={choosePack} onDownload={(p) => void download(p)} onRemove={(p) => void remove(p)} />
        </Popover>
      )}
    </div>
  );
}
