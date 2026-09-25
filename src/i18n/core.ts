/**
 * The pure half of the app's translations: catalogs as flat maps, plural keys, placeholders and
 * the lookup that falls back to English. No React and no globals, so it's tested on its own.
 *
 * The catalogs are i18next's JSON: one file per namespace, strings or objects of more of them,
 * `{{name}}` placeholders, and a plural as one key per CLDR category (`files_one`, `files_other`,
 * and `_few` or `_many` where the language has them), chosen with `Intl.PluralRules`.
 */

/** A namespace's JSON as translators write it: strings, or objects of more of them. */
export type CatalogTree = { [key: string]: string | CatalogTree };

/** Every message by its full key: "sidebar.addPhoto", "folder.count_one". */
export type Catalog = Record<string, string>;

/** What fills a message's placeholders. A number is written the way the language writes numbers. */
export type Vars = Record<string, string | number>;

/** Every plural category CLDR has; a language uses some of them. */
export const PLURAL_CATEGORIES = ["zero", "one", "two", "few", "many", "other"] as const;

export type PluralCategory = (typeof PLURAL_CATEGORIES)[number];

/** A placeholder: `{{name}}`, spaces inside the braces allowed. */
export const PLACEHOLDER = /\{\{\s*([A-Za-z0-9_]+)\s*\}\}/g;

/**
 * A namespace's tree as flat keys under `prefix`: `{"empty": {"title": "…"}}` in `library` is
 * `library.empty.title`. Anything that isn't a string or an object is left out.
 */
export function flatten(tree: unknown, prefix: string, out: Catalog = {}): Catalog {
  if (typeof tree !== "object" || tree === null || Array.isArray(tree)) return out;
  for (const [key, value] of Object.entries(tree)) {
    const full = prefix ? `${prefix}.${key}` : key;
    if (typeof value === "string") out[full] = value;
    else flatten(value, full, out);
  }
  return out;
}

const pluralRules = new Map<string, Intl.PluralRules>();

/** The CLDR plural category `count` takes in `locale`: "one" for 1 in English, "other" for anything in Japanese. */
export function pluralCategory(locale: string, count: number): PluralCategory {
  let rules = pluralRules.get(locale);
  if (!rules) {
    rules = new Intl.PluralRules(locale);
    pluralRules.set(locale, rules);
  }
  return rules.select(count) as PluralCategory;
}

/** The plural categories a language has, as its catalog's plural keys must: `["one", "other"]` for English. */
export function pluralCategories(locale: string): PluralCategory[] {
  const have = new Intl.PluralRules(locale).resolvedOptions().pluralCategories as PluralCategory[];
  return PLURAL_CATEGORIES.filter((c) => have.includes(c));
}

const numberFormats = new Map<string, Intl.NumberFormat>();

/** A number as `locale` writes it: 1,234 in English, 1 234 in French. */
export function formatNumberIn(locale: string, value: number): string {
  let format = numberFormats.get(locale);
  if (!format) {
    format = new Intl.NumberFormat(locale);
    numberFormats.set(locale, format);
  }
  return format.format(value);
}

/**
 * `message` with its placeholders filled. Numbers are written the way `intlLocale` writes them; a
 * placeholder with no value is left as it is, so a missing one shows rather than vanishing.
 */
export function interpolate(message: string, vars: Vars | undefined, intlLocale: string): string {
  if (!vars || !message.includes("{{")) return message;
  return message.replace(PLACEHOLDER, (whole, name: string) => {
    const value = vars[name];
    if (value === undefined || value === null) return whole;
    return typeof value === "number" ? formatNumberIn(intlLocale, value) : String(value);
  });
}

/** Where a lookup looks: the language on show, and English under it. */
export type Lookup = {
  /** The language on show, whose plural rules pick a plural key. */
  locale: string;
  /** The locale numbers are written in. */
  intl: string;
  active: Catalog;
  /** English: what a message missing from `active` falls back to. */
  fallback: Catalog;
};

/**
 * The message for `key`, placeholders filled. With a numeric `count` it's a plural: the key for the
 * category `count` takes in the language on show, then that language's `_other`, then English's own
 * category and `_other`. Anything missing from the language on show comes from English; a key
 * English doesn't have either comes back as the key itself, and `missing` hears about it.
 */
export function translate(lookup: Lookup, key: string, vars?: Vars, missing?: (key: string) => void): string {
  const { active, fallback } = lookup;
  let message: string | undefined;
  const count = vars?.count;
  if (typeof count === "number") {
    message =
      active[`${key}_${pluralCategory(lookup.locale, count)}`] ??
      active[`${key}_other`] ??
      fallback[`${key}_${pluralCategory("en", count)}`] ??
      fallback[`${key}_other`];
  }
  message ??= active[key] ?? fallback[key] ?? active[`${key}_other`] ?? fallback[`${key}_other`];
  if (message === undefined) {
    missing?.(key);
    return key;
  }
  return interpolate(message, vars, lookup.intl);
}

/** The placeholders a message uses, each once, in order. */
export function placeholders(message: string): string[] {
  return [...new Set([...message.matchAll(PLACEHOLDER)].map((m) => m[1]))];
}

/** A piece of a message with markup: text, or a tag around some text (`<b>…</b>`) or alone (`<brand/>`). */
export type RichPart = { text: string } | { tag: string; text: string };

const TAG = /<([a-z][a-z0-9]*)\s*\/>|<([a-z][a-z0-9]*)>([\s\S]*?)<\/\2>/g;

/**
 * A message split at its tags, for the few sentences with a word in bold or a link inside them:
 * "Your packs are credited to <b>{{handle}}</b>." is text, a `b` around the handle, and text.
 * Tags don't nest; a tag the message doesn't close is left as text.
 */
export function richParts(message: string): RichPart[] {
  const parts: RichPart[] = [];
  let at = 0;
  for (const m of message.matchAll(TAG)) {
    if (m.index > at) parts.push({ text: message.slice(at, m.index) });
    parts.push(m[1] ? { tag: m[1], text: "" } : { tag: m[2], text: m[3] });
    at = m.index + m[0].length;
  }
  if (at < message.length) parts.push({ text: message.slice(at) });
  return parts;
}

/** The tags a message uses, each once, in order: what a translation must keep. */
export function tags(message: string): string[] {
  return [...new Set(richParts(message).flatMap((p) => ("tag" in p ? [p.tag] : [])))];
}
