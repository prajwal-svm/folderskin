/**
 * Icon packs as the composer reads them (scripts/icon-packs.mjs writes them): the pack, its
 * icons' path data, and searching them. Pure, so it's tested without a canvas.
 */
import type { IconDrawing } from "../doc";

/** One icon as a pack stores it, with short keys to keep packs small. */
export type IconDef = {
  /** Its name, "arrow-big-up". */
  n: string;
  /** Path data in the pack's square grid. */
  d: string[];
  /** Words people search for it by. */
  t?: string[];
  /** Line packs: which paths are filled shapes. */
  f?: number[];
  /** Filled with the even-odd rule. */
  r?: 1;
  /** Its own colour, such as a brand's. */
  c?: string;
};

export type IconPack = {
  format: 1;
  id: string;
  name: string;
  version: string;
  license: string;
  source: string;
  style: "stroke" | "fill";
  viewBox: number;
  strokeWidth: number;
  /** Logos that belong to their owners. */
  brands: boolean;
  icons: IconDef[];
};

const PATH = /^[MmLlHhVvCcSsQqTtAaZz0-9eE.,+\-\s]+$/;
const isStrings = (v: unknown): v is string[] => Array.isArray(v) && v.every((x) => typeof x === "string");

/** A pack read from a file, checked: `null` when it isn't one. Icons that don't check out are left out. */
export function parsePack(value: unknown): IconPack | null {
  const v = typeof value === "string" ? safeJson(value) : value;
  if (!v || typeof v !== "object") return null;
  const p = v as Record<string, unknown>;
  if (p.format !== 1 || typeof p.id !== "string" || typeof p.name !== "string" || !Array.isArray(p.icons)) return null;
  const viewBox = typeof p.viewBox === "number" && p.viewBox > 0 && p.viewBox <= 1024 ? p.viewBox : 24;
  const icons: IconDef[] = [];
  for (const raw of p.icons) {
    if (!raw || typeof raw !== "object") continue;
    const i = raw as Record<string, unknown>;
    if (typeof i.n !== "string" || !isStrings(i.d) || i.d.length === 0 || !i.d.every((d) => PATH.test(d))) continue;
    const def: IconDef = { n: i.n, d: i.d };
    if (isStrings(i.t)) def.t = i.t;
    if (Array.isArray(i.f)) def.f = i.f.filter((x): x is number => Number.isInteger(x));
    if (i.r === 1) def.r = 1;
    if (typeof i.c === "string" && /^#[0-9a-f]{6}$/i.test(i.c)) def.c = i.c;
    icons.push(def);
  }
  return {
    format: 1,
    id: p.id,
    name: p.name,
    version: typeof p.version === "string" ? p.version : "",
    license: typeof p.license === "string" ? p.license : "",
    source: typeof p.source === "string" ? p.source : "",
    style: p.style === "fill" ? "fill" : "stroke",
    viewBox,
    strokeWidth: typeof p.strokeWidth === "number" && p.strokeWidth >= 0 ? p.strokeWidth : 2,
    brands: p.brands === true,
    icons,
  };
}

function safeJson(s: string): unknown {
  try {
    return JSON.parse(s);
  } catch {
    return null;
  }
}

/** What a layer keeps of an icon: its drawing, so the design doesn't need the pack to open. */
export function drawingOf(pack: IconPack, icon: IconDef): IconDrawing {
  return {
    pack: pack.id,
    icon: icon.n,
    paths: icon.d,
    filled: icon.f,
    style: pack.style,
    viewBox: pack.viewBox,
    strokeWidth: pack.strokeWidth,
    evenOdd: icon.r === 1,
    brand: icon.c,
  };
}

/** Each icon's words in lower case, worked out once per pack rather than on every keystroke. */
export type IconIndex = { names: string[]; words: string[] };

export function indexPack(pack: IconPack): IconIndex {
  return {
    names: pack.icons.map((i) => i.n.toLowerCase().replace(/[-_]+/g, " ")),
    words: pack.icons.map((i) => (i.t ?? []).join(" ").toLowerCase()),
  };
}

/**
 * The icons matching `query`, best first: the name itself, then names starting with it, names
 * containing it, and last the words people search by. Every word typed has to match somewhere.
 * An empty query is the whole pack, in name order.
 */
export function searchIcons(pack: IconPack, index: IconIndex, query: string): IconDef[] {
  const words = query.toLowerCase().replace(/[-_]+/g, " ").trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return pack.icons;
  const phrase = words.join(" ");
  const scored: { icon: IconDef; score: number; i: number }[] = [];
  pack.icons.forEach((icon, i) => {
    const name = index.names[i];
    const tags = index.words[i];
    let score = 0;
    for (const w of words) {
      if (name.split(" ").includes(w)) score += 6;
      else if (name.startsWith(w) || name.includes(` ${w}`)) score += 4;
      else if (name.includes(w)) score += 3;
      else if (tags.includes(w)) score += 1;
      else return;
    }
    if (name === phrase) score += 20;
    else if (name.startsWith(phrase)) score += 8;
    scored.push({ icon, score, i });
  });
  scored.sort((a, b) => b.score - a.score || a.i - b.i);
  return scored.map((s) => s.icon);
}
