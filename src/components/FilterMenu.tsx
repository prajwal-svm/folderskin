import { type CSSProperties, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  activeCount,
  applyFilters,
  type Facet,
  type FacetId,
  facets,
  type FilterContext,
  type Filters,
  NO_FILTERS,
  type Sort,
  SORTS,
  toggleChoice,
} from "../lib/filters";
import type { Skin } from "../lib/tauri";
import { CheckIcon } from "./icons/check";
import { ListFilterIcon } from "./icons/list-filter";
import { StarIcon } from "./icons/star";

const WIDTH = 320;
const MARGIN = 12;

/**
 * The filter button beside the search, and the popover it opens: the order skins are listed in,
 * favourites only, and a section for each thing skins can be told apart by (where they came from,
 * their pack, colour, brightness, when they were added, and who or what made them). Only the
 * sections that can narrow what's in view are offered. The button shows how many choices are on.
 */
export function FilterMenu({
  skins,
  filters,
  onFilters,
  sort,
  onSort,
  ctx,
  showFavourites,
  reading,
}: {
  /** What's in view before any filter: the whole library, yours, or favourites. */
  skins: Skin[];
  filters: Filters;
  onFilters: (filters: Filters) => void;
  sort: Sort;
  onSort: (sort: Sort) => void;
  ctx: FilterContext;
  /** False in Favourites, where every skin has a star anyway. */
  showFavourites: boolean;
  /** Colours are still being read from some of the pictures. */
  reading: boolean;
}) {
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const count = activeCount(filters);
  const close = useCallback(() => setOpen(false), []);

  return (
    <>
      <button
        ref={button}
        type="button"
        className={count ? "filter-btn is-on" : "filter-btn"}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={count ? `Filters, ${count} on` : "Filters"}
        title="Filters"
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => setOpen((o) => !o)}
      >
        <ListFilterIcon size={16} />
        {count > 0 && <span className="filter-btn-count">{count}</span>}
      </button>
      {open && button.current && (
        <FilterPopover
          anchor={button.current}
          skins={skins}
          filters={filters}
          onFilters={onFilters}
          sort={sort}
          onSort={onSort}
          ctx={ctx}
          showFavourites={showFavourites}
          reading={reading}
          onClose={close}
        />
      )}
    </>
  );
}

function FilterPopover({
  anchor,
  skins,
  filters,
  onFilters,
  sort,
  onSort,
  ctx,
  showFavourites,
  reading,
  onClose,
}: {
  anchor: HTMLElement;
  skins: Skin[];
  filters: Filters;
  onFilters: (filters: Filters) => void;
  sort: Sort;
  onSort: (sort: Sort) => void;
  ctx: FilterContext;
  showFavourites: boolean;
  reading: boolean;
  onClose: () => void;
}) {
  const panel = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number; maxHeight: number } | null>(null);
  const shown = useMemo(() => facets(skins, filters, ctx), [skins, filters, ctx]);
  const matching = useMemo(() => applyFilters(skins, filters, ctx).length, [skins, filters, ctx]);
  const count = activeCount(filters);

  // Under the button, its right edge lined up with the button's, inside the window.
  useLayoutEffect(() => {
    const place = () => {
      const a = anchor.getBoundingClientRect();
      const left = Math.min(Math.max(MARGIN, a.right - WIDTH), window.innerWidth - WIDTH - MARGIN);
      const top = a.bottom + 8;
      setPos({ left, top, maxHeight: Math.max(160, window.innerHeight - top - MARGIN) });
    };
    place();
    window.addEventListener("resize", place);
    return () => window.removeEventListener("resize", place);
  }, [anchor]);

  // Keyboard users land in the popover; Tab goes on through its controls.
  useEffect(() => panel.current?.focus({ preventScroll: true }), []);

  useEffect(() => {
    const down = (e: MouseEvent) => {
      const target = e.target as Node;
      if (!panel.current?.contains(target) && !anchor.contains(target)) onClose();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      onClose();
      anchor.focus({ preventScroll: true });
    };
    document.addEventListener("mousedown", down);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("mousedown", down);
      document.removeEventListener("keydown", key);
    };
  }, [anchor, onClose]);

  const choose = (facet: Facet, value: string) => onFilters(toggleChoice(filters, facet.id, value, facet.single));

  return createPortal(
    <div
      ref={panel}
      className="filter-pop"
      role="dialog"
      aria-label="filter and sort skins"
      tabIndex={-1}
      style={pos ? { left: pos.left, top: pos.top, width: WIDTH, maxHeight: pos.maxHeight } : { visibility: "hidden" }}
    >
      <div className="filter-head">
        <div>
          <p className="filter-title">Filters</p>
          <p className="filter-sub">{count ? `${matching} of ${skins.length} skins` : `${skins.length} skins`}</p>
        </div>
        {count > 0 && (
          <button type="button" className="filter-clear" onClick={() => onFilters(NO_FILTERS)}>
            Clear all
          </button>
        )}
      </div>

      <section className="filter-sec">
        <p className="filter-label">Sort by</p>
        <div className="seg seg-sm filter-sort" role="radiogroup" aria-label="sort by">
          {SORTS.map((s) => (
            <button
              key={s.id}
              type="button"
              role="radio"
              aria-checked={sort === s.id}
              className={sort === s.id ? "seg-btn is-active" : "seg-btn"}
              onClick={() => onSort(s.id)}
            >
              {s.label}
            </button>
          ))}
        </div>
      </section>

      {showFavourites && (
        <button
          type="button"
          role="switch"
          aria-checked={filters.favourites}
          className="filter-fave"
          onClick={() => onFilters({ ...filters, favourites: !filters.favourites })}
        >
          <StarIcon size={15} />
          <span>Favourites only</span>
          <span className={filters.favourites ? "switch is-on" : "switch"} aria-hidden="true">
            <span className="knob" />
          </span>
        </button>
      )}

      {shown.map((facet) => (
        <section className="filter-sec" key={facet.id}>
          <p className="filter-label">
            {facet.label}
            {facet.id === "colour" && reading && <span className="filter-note"> · still reading colours…</span>}
          </p>
          {facet.id === "colour" ? (
            <div className="filter-swatches">
              {facet.options.map((o) => (
                <Swatch key={o.value} option={o} on={isOn(filters, facet.id, o.value)} onClick={() => choose(facet, o.value)} />
              ))}
            </div>
          ) : (
            <div className="filter-chips">
              {facet.options.map((o) => {
                const on = isOn(filters, facet.id, o.value);
                return (
                  <button
                    key={o.value}
                    type="button"
                    aria-pressed={on}
                    className={on ? "fchip is-on" : "fchip"}
                    disabled={!on && o.count === 0}
                    onClick={() => choose(facet, o.value)}
                  >
                    <span className="fchip-label">{o.label}</span>
                    <span className="count">{o.count}</span>
                  </button>
                );
              })}
            </div>
          )}
        </section>
      ))}

      {shown.length === 0 && !reading && (
        <p className="filter-empty">These skins are all alike so far. Add another pack or a picture of your own and there'll be more to filter by.</p>
      )}
    </div>,
    document.body,
  );
}

const isOn = (filters: Filters, facet: FacetId, value: string) => filters.chosen[facet]?.includes(value) ?? false;

function Swatch({ option, on, onClick }: { option: { value: string; label: string; count: number; swatch?: string }; on: boolean; onClick: () => void }) {
  const light = option.value === "white" || option.value === "yellow";
  return (
    <button
      type="button"
      aria-pressed={on}
      aria-label={`${option.label}, ${option.count}`}
      title={`${option.label} · ${option.count}`}
      className={[on ? "swatch is-on" : "swatch", light ? "is-light" : ""].filter(Boolean).join(" ")}
      style={{ "--swatch": option.swatch } as CSSProperties}
      disabled={!on && option.count === 0}
      onClick={onClick}
    >
      {on && <CheckIcon size={13} playOnMount />}
    </button>
  );
}
