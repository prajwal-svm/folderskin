/**
 * The library's filters and sort order, behind the filter button beside the search. Each filter
 * is a facet (where a skin came from, its pack, its colours and so on). Choices in one facet widen
 * it (any of them); facets narrow each other (all of them). A facet's counts say how many skins
 * each choice would show with the other facets as they are.
 */
import type { Skin } from "./tauri";
import { COLOURS, type Palette } from "./palette";
import { licenseLabel } from "./packs";

export type Sort = "newest" | "oldest" | "az" | "za";

export const SORTS: { id: Sort; label: string }[] = [
  { id: "newest", label: "Newest" },
  { id: "oldest", label: "Oldest" },
  { id: "az", label: "A–Z" },
  { id: "za", label: "Z–A" },
];

export type FacetId = "source" | "pack" | "colour" | "tone" | "added" | "model" | "author" | "license";

export type Filters = {
  /** Only the skins with a star. */
  favourites: boolean;
  /** For each facet, the choices made in it. */
  chosen: Partial<Record<FacetId, string[]>>;
};

export const NO_FILTERS: Filters = { favourites: false, chosen: {} };

/** Everything a skin's facets are read from besides the skin itself. */
export type FilterContext = {
  favourites: ReadonlySet<string>;
  palettes: ReadonlyMap<string, Palette>;
  /** Now, in Unix ms: "added today" is relative to it. */
  now: number;
};

export type FacetOption = { value: string; label: string; count: number; swatch?: string };

export type Facet = {
  id: FacetId;
  label: string;
  /** Some facets take one choice at a time (a skin is added once, and is light or dark). */
  single: boolean;
  options: FacetOption[];
};

const DAY = 24 * 60 * 60 * 1000;

/** When a skin was added, as the choices the Added facet offers, newest first. */
const ADDED = [
  { value: "today", label: "Today" },
  { value: "week", label: "Past week" },
  { value: "month", label: "Past month" },
  { value: "older", label: "Older" },
];

const SOURCES: Record<string, string> = { community: "Community packs", import: "Your pictures", ai: "Made with AI", composer: "Your designs" };

type FacetDef = {
  id: FacetId;
  label: string;
  single?: boolean;
  /** The values a skin has in this facet: none when it doesn't apply to the skin. */
  values: (skin: Skin, ctx: FilterContext) => string[];
  /** The label of a value; the value itself when missing. */
  label_of?: (value: string, skins: Skin[]) => string;
  /** The order values are listed in: by count unless given. */
  order?: string[];
  swatch?: (value: string) => string | undefined;
};

/** Which of the Added choices a skin falls in: every window it's inside, so "Past week" includes today. */
function addedValues(skin: Skin, now: number): string[] {
  if (!skin.created_at) return [];
  const midnight = new Date(now);
  midnight.setHours(0, 0, 0, 0);
  const age = now - skin.created_at;
  if (skin.created_at >= midnight.getTime()) return ["today", "week", "month"];
  if (age < 7 * DAY) return ["week", "month"];
  if (age < 30 * DAY) return ["month"];
  return ["older"];
}

const FACETS: FacetDef[] = [
  {
    id: "source",
    label: "From",
    values: (s) => (s.source ? [s.source] : []),
    label_of: (v) => SOURCES[v] ?? v,
    order: Object.keys(SOURCES),
  },
  {
    id: "pack",
    label: "Pack",
    values: (s) => (s.pack ? [s.pack] : []),
    label_of: (v, skins) => skins.find((s) => s.pack === v)?.pack_name ?? v,
  },
  {
    id: "colour",
    label: "Colour",
    values: (s, ctx) => ctx.palettes.get(s.id)?.colours ?? [],
    label_of: (v) => COLOURS.find((c) => c.id === v)?.label ?? v,
    order: COLOURS.map((c) => c.id),
    swatch: (v) => COLOURS.find((c) => c.id === v)?.swatch,
  },
  {
    id: "tone",
    label: "Brightness",
    single: true,
    values: (s, ctx) => {
      const tone = ctx.palettes.get(s.id)?.tone;
      return tone ? [tone] : [];
    },
    label_of: (v) => (v === "light" ? "Light" : "Dark"),
    order: ["light", "dark"],
  },
  {
    id: "added",
    label: "Added",
    single: true,
    values: (s, ctx) => addedValues(s, ctx.now),
    label_of: (v) => ADDED.find((a) => a.value === v)?.label ?? v,
    order: ADDED.map((a) => a.value),
  },
  {
    id: "model",
    label: "Made with",
    values: (s) => (s.made_with ? [s.made_with] : []),
  },
  {
    id: "author",
    label: "Author",
    values: (s) => (s.author ? [s.author] : []),
    label_of: (v) => `@${v}`,
  },
  {
    id: "license",
    label: "Licence",
    values: (s) => (s.license ? [s.license] : []),
    label_of: licenseLabel,
  },
];

/** How many choices are made, the favourites switch included: the number on the filter button. */
export function activeCount(filters: Filters): number {
  return (filters.favourites ? 1 : 0) + Object.values(filters.chosen).reduce((n, values) => n + (values?.length ?? 0), 0);
}

function passes(skin: Skin, filters: Filters, ctx: FilterContext, skip?: FacetId): boolean {
  if (filters.favourites && !ctx.favourites.has(skin.id)) return false;
  for (const def of FACETS) {
    if (def.id === skip) continue;
    const chosen = filters.chosen[def.id];
    if (!chosen?.length) continue;
    const values = def.values(skin, ctx);
    if (!chosen.some((c) => values.includes(c))) return false;
  }
  return true;
}

/** The skins every filter lets through. */
export function applyFilters(skins: Skin[], filters: Filters, ctx: FilterContext): Skin[] {
  return activeCount(filters) === 0 ? skins : skins.filter((s) => passes(s, filters, ctx));
}

/**
 * The facets worth offering for `skins`: each with its choices and how many skins each would
 * show. A facet with fewer than two choices, or whose every choice every skin has, is left out,
 * unless something in it is chosen, so it can be unchosen.
 */
export function facets(skins: Skin[], filters: Filters, ctx: FilterContext): Facet[] {
  const out: Facet[] = [];
  for (const def of FACETS) {
    const chosen = filters.chosen[def.id] ?? [];
    // What the other facets let through, which this facet's choices would narrow.
    const pool = skins.filter((s) => passes(s, filters, ctx, def.id));
    const counts = new Map<string, number>();
    for (const skin of pool) for (const v of def.values(skin, ctx)) counts.set(v, (counts.get(v) ?? 0) + 1);
    // One choice isn't a choice, and neither are choices every skin has.
    const narrows = counts.size > 1 && [...counts.values()].some((n) => n < pool.length);
    if (!chosen.length && !narrows) continue;
    for (const c of chosen) if (!counts.has(c)) counts.set(c, 0);
    const order = def.order;
    const options = [...counts]
      .map(([value, count]) => ({
        value,
        count,
        label: def.label_of ? def.label_of(value, skins) : value,
        swatch: def.swatch?.(value),
      }))
      .sort((a, b) =>
        order ? order.indexOf(a.value) - order.indexOf(b.value) : b.count - a.count || a.label.localeCompare(b.label),
      );
    out.push({ id: def.id, label: def.label, single: def.single ?? false, options });
  }
  return out;
}

/** Makes or unmakes one choice. In a single-choice facet it replaces the one made before. */
export function toggleChoice(filters: Filters, facet: FacetId, value: string, single = false): Filters {
  const now = filters.chosen[facet] ?? [];
  const next = now.includes(value) ? now.filter((v) => v !== value) : single ? [value] : [...now, value];
  const chosen = { ...filters.chosen, [facet]: next };
  if (!next.length) delete chosen[facet];
  return { ...filters, chosen };
}

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

/** The skins in the chosen order. Newest keeps the library's own order, where a pack stays in its order. */
export function sortSkins(skins: Skin[], sort: Sort): Skin[] {
  switch (sort) {
    case "newest":
      return skins;
    case "oldest":
      return [...skins].reverse();
    case "az":
      return [...skins].sort((a, b) => collator.compare(a.name, b.name));
    case "za":
      return [...skins].sort((a, b) => collator.compare(b.name, a.name));
  }
}

/** Whether a skin matches what's typed in the search: its name, tags, pack, author or AI idea. */
export function matchesQuery(skin: Skin, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  const fields = [skin.name, skin.pack_name, skin.author, skin.idea, skin.made_with, ...skin.tags];
  return fields.some((f) => f?.toLowerCase().includes(q));
}

const SORT_KEY = "folderskin.sort";

/** The sort order chosen last time, kept in this browser only. */
export function loadSort(): Sort {
  try {
    const saved = localStorage.getItem(SORT_KEY);
    return SORTS.some((s) => s.id === saved) ? (saved as Sort) : "newest";
  } catch {
    return "newest";
  }
}

export function saveSort(sort: Sort): void {
  try {
    localStorage.setItem(SORT_KEY, sort);
  } catch {
    // The order goes back to newest next time; nothing else depends on it.
  }
}
