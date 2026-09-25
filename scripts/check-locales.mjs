#!/usr/bin/env node
/**
 * Checks the app's translations (src/locales/<code>/<namespace>.json) against its English, the way
 * src/locales/TRANSLATORS.md describes them.
 *
 *   pnpm check:locales                        every language. A namespace not translated yet ({}) is a warning
 *   pnpm check:locales --strict               the same, and a namespace not translated yet fails too
 *   pnpm check:locales --locale fr --strict   one language, as a translator checks their work
 *
 * For every language, English included:
 *   - each namespace is valid JSON of strings and objects, and no message is empty
 *   - no message has an em dash, a horizontal bar or a semicolon (— ― –– ； ;), in any language
 *   - placeholders ({{name}}) and tags (<b>…</b>, <brand/>) are whole, not broken
 * For every language but English:
 *   - it has the same namespaces as English, and a namespace it has started has every key English
 *     has, and none English doesn't
 *   - a plural has exactly the forms the language's plural rules use (CLDR, through Intl): one and
 *     other in English, other alone in Chinese, Japanese and Korean, one, many and other in French
 *     and Spanish
 *   - each message uses the same placeholders and tags as its English. A plural form may leave out
 *     {{count}} ("Un dossier"), except its "other" form
 *
 * Exits 1 when anything fails. Nothing here depends on anything outside Node itself.
 */
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

/** Where the catalogs are: one folder per language, one file per namespace. */
export const LOCALES_DIR = join(ROOT, "src", "locales");

/** Every plural category CLDR has, in its order. A language uses some of them. */
export const PLURAL_CATEGORIES = ["zero", "one", "two", "few", "many", "other"];

const PLURAL_SUFFIX = new RegExp(`_(${PLURAL_CATEGORIES.join("|")})$`);

/** A placeholder, as src/i18n/core.ts reads it. */
const PLACEHOLDER = /\{\{\s*([A-Za-z0-9_]+)\s*\}\}/g;

/** A tag, as src/i18n/core.ts reads it: alone (`<brand/>`) or around some text (`<b>…</b>`). */
const TAG = /<([a-z][a-z0-9]*)\s*\/>|<([a-z][a-z0-9]*)>([\s\S]*?)<\/\2>/g;

/**
 * What no message may have, in any language (the glossary's rule): dashes standing in for a pause,
 * and semicolons. A sentence that wants one is two sentences, or takes a colon, a comma or brackets.
 */
export const FORBIDDEN = [
  ["—", "an em dash (—)"],
  ["―", "a horizontal bar (―)"],
  ["––", "a double en dash (––)"],
  [";", "a semicolon (;)"],
  ["；", "a full-width semicolon (；)"],
];

/**
 * The languages the app speaks, read from src/i18n/locales.ts so there is one list. English is
 * first: it's what the others are checked against.
 */
export function readLocales(file = join(ROOT, "src", "i18n", "locales.ts")) {
  const source = readFileSync(file, "utf8");
  const list = /export const LOCALES = \[([^\]]*)\]/.exec(source);
  if (!list) throw new Error(`${file} has no LOCALES list`);
  return [...list[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

/** The plural categories `locale` uses, in CLDR's order: ["one", "other"] for English. */
export function pluralCategories(locale) {
  const have = new Intl.PluralRules(locale).resolvedOptions().pluralCategories;
  return PLURAL_CATEGORIES.filter((c) => have.includes(c));
}

/** A key without its plural suffix, and the suffix: "count_one" is ["count", "one"]. */
function splitPlural(key) {
  const m = PLURAL_SUFFIX.exec(key);
  return m ? [key.slice(0, m.index), m[1]] : [key, null];
}

/**
 * A namespace's JSON as flat keys: `{"empty": {"title": "…"}}` is `empty.title`. Anything that isn't
 * a string or an object of more of them is a problem, and so is a key with a dot in it (the app
 * reads a dot as a step into an object).
 */
export function flatten(tree, problems, prefix = "", out = {}) {
  if (typeof tree !== "object" || tree === null || Array.isArray(tree)) {
    problems.push({ key: prefix || undefined, message: "has to be an object of messages" });
    return out;
  }
  for (const [key, value] of Object.entries(tree)) {
    const full = prefix ? `${prefix}.${key}` : key;
    if (key.includes(".") || key.trim() === "") problems.push({ key: full, message: "a key can't be empty or have a dot in it" });
    else if (typeof value === "string") out[full] = value;
    else if (typeof value === "object" && value !== null && !Array.isArray(value)) flatten(value, problems, full, out);
    else problems.push({ key: full, message: `has to be a string, not ${Array.isArray(value) ? "a list" : value === null ? "null" : typeof value}` });
  }
  return out;
}

/** The placeholders a message uses, each once, sorted. */
export function placeholdersOf(message) {
  return [...new Set([...message.matchAll(PLACEHOLDER)].map((m) => m[1]))].sort();
}

/** The tags a message uses, each once, sorted. */
export function tagsOf(message) {
  return [...new Set([...message.matchAll(TAG)].map((m) => m[1] ?? m[2]))].sort();
}

const list = (names, wrap) => (names.length ? names.map(wrap).join(", ") : "none");
const braces = (n) => `{{${n}}}`;
const angles = (n) => `<${n}>`;

/** What's wrong with one message on its own, whatever its language. */
export function checkMessage(message) {
  const out = [];
  if (message.trim() === "") out.push("is empty");
  for (const [text, name] of FORBIDDEN) if (message.includes(text)) out.push(`has ${name}. Rewrite the sentence without it`);
  const unplaced = message.replace(PLACEHOLDER, "");
  if (unplaced.includes("{{") || unplaced.includes("}}")) out.push("has a broken placeholder. Write it as {{name}}, exactly as in English");
  const untagged = message.replace(TAG, (whole, alone, open, inner) => (alone ? "" : inner));
  if (/<\/?[a-z][a-z0-9]*\s*\/?>/.test(untagged)) out.push("has a tag that isn't closed, or is closed in the wrong place");
  return out;
}

/**
 * Checks one language's namespaces against English's. `english` and `catalogs` map a namespace's
 * name to its parsed JSON. In `catalogs`, `undefined` is a file that isn't there and an `Error` is
 * one that didn't parse. Returns the problems, each `{ level, namespace, key?, message }`, and how
 * many of English's messages the language has.
 */
export function checkLocale(locale, english, catalogs, { strict = false } = {}) {
  const problems = [];
  const categories = pluralCategories(locale);
  let messages = 0;
  let translated = 0;
  const report = (level, namespace, key, message) => problems.push({ level, namespace, key, message });

  for (const namespace of Object.keys(catalogs)) {
    if (!(namespace in english)) report("error", namespace, undefined, "English has no namespace by this name. Remove the file");
  }

  for (const [namespace, englishTree] of Object.entries(english)) {
    const flatProblems = [];
    const source = flatten(englishTree, flatProblems);
    // The English messages, plurals by their base key: {count: {one: "…", other: "…"}}.
    const plurals = new Map();
    const singles = new Map();
    for (const [key, value] of Object.entries(source)) {
      const [base, category] = splitPlural(key);
      if (category) plurals.set(base, { ...plurals.get(base), [category]: value });
      else singles.set(key, value);
    }
    messages += plurals.size + singles.size;

    const tree = catalogs[namespace];
    if (tree === undefined) {
      report("error", namespace, undefined, `is missing. Copy it from English, or write {} until it's translated`);
      continue;
    }
    if (tree instanceof Error) {
      report("error", namespace, undefined, `isn't valid JSON: ${tree.message}`);
      continue;
    }
    const ownProblems = [];
    const own = flatten(tree, ownProblems);
    for (const p of ownProblems) report("error", namespace, p.key, p.message);
    const keys = Object.keys(own);
    if (keys.length === 0 && ownProblems.length === 0) {
      const untranslated = plurals.size + singles.size;
      problems.push({ level: strict ? "error" : "warning", namespace, message: `isn't translated yet: its ${untranslated} messages show in English`, untranslated });
      continue;
    }

    for (const [key, value] of Object.entries(own)) {
      for (const message of checkMessage(value)) report("error", namespace, key, message);
    }

    const seen = new Set();
    for (const [key, englishValue] of singles) {
      seen.add(key);
      const value = own[key];
      if (value === undefined) {
        const plural = categories.map((c) => `${key}_${c}`).filter((k) => k in own);
        report("error", namespace, key, plural.length ? `isn't a plural in English. Write it as ${key}, with no _${splitPlural(plural[0])[1]}` : `is missing. English: "${englishValue}"`);
        plural.forEach((k) => seen.add(k));
        continue;
      }
      translated++;
      compare(namespace, key, value, englishValue);
    }

    for (const [base, forms] of plurals) {
      const englishOther = forms.other ?? Object.values(forms)[0];
      const allowed = [...new Set(Object.values(forms).flatMap(placeholdersOf))].sort();
      let complete = true;
      for (const category of categories) {
        const key = `${base}_${category}`;
        seen.add(key);
        const value = own[key];
        if (value === undefined) {
          complete = false;
          const hint = base in own ? `. It's a plural: write ${categories.map((c) => `${base}_${c}`).join(" and ")}, not ${base}` : "";
          report("error", namespace, key, `is missing${hint || `. English: "${englishOther}"`}`);
          continue;
        }
        const englishValue = forms[category] ?? englishOther;
        comparePlural(namespace, key, category, value, englishValue, allowed, englishOther);
      }
      if (base in own) seen.add(base);
      if (complete) translated++;
      for (const category of PLURAL_CATEGORIES) {
        const key = `${base}_${category}`;
        if (!categories.includes(category) && key in own) {
          seen.add(key);
          report("error", namespace, key, `${locale} has no "${category}" plural form. It uses ${categories.map((c) => `_${c}`).join(", ")}. Remove this one`);
        }
      }
    }

    for (const key of keys) {
      if (!seen.has(key)) report("error", namespace, key, "English has no message by this key. Remove it, or check its spelling");
    }
  }

  function compare(namespace, key, value, englishValue) {
    const want = placeholdersOf(englishValue);
    const have = placeholdersOf(value);
    if (want.join() !== have.join()) {
      report("error", namespace, key, `uses the placeholders ${list(have, braces)}, and English uses ${list(want, braces)}. Keep them exactly, untranslated`);
    }
    compareTags(namespace, key, value, englishValue);
  }

  function comparePlural(namespace, key, category, value, englishValue, allowed, englishOther) {
    const have = placeholdersOf(value);
    const extra = have.filter((n) => !allowed.includes(n));
    const needed = allowed.filter((n) => n !== "count" || (category === "other" && placeholdersOf(englishOther).includes("count")));
    const lacking = needed.filter((n) => !have.includes(n));
    if (extra.length) report("error", namespace, key, `uses ${list(extra, braces)}, which English doesn't. Keep the placeholders exactly, untranslated`);
    if (lacking.length) report("error", namespace, key, `leaves out ${list(lacking, braces)}, which English uses`);
    compareTags(namespace, key, value, englishValue);
  }

  function compareTags(namespace, key, value, englishValue) {
    const want = tagsOf(englishValue);
    const have = tagsOf(value);
    if (want.join() !== have.join()) {
      report("error", namespace, key, `uses the tags ${list(have, angles)}, and English uses ${list(want, angles)}. Keep each tag, with its closing tag, around the words it goes with`);
    }
  }

  return { problems, messages, translated };
}

/** Checks English on its own: the same rules for every message, and plurals with English's forms. */
export function checkEnglish(english) {
  const problems = [];
  const categories = pluralCategories("en");
  for (const [namespace, tree] of Object.entries(english)) {
    if (tree instanceof Error) {
      problems.push({ level: "error", namespace, message: `isn't valid JSON: ${tree.message}` });
      continue;
    }
    const flatProblems = [];
    const flat = flatten(tree, flatProblems);
    for (const p of flatProblems) problems.push({ level: "error", namespace, key: p.key, message: p.message });
    const plurals = new Map();
    for (const [key, value] of Object.entries(flat)) {
      for (const message of checkMessage(value)) problems.push({ level: "error", namespace, key, message });
      const [base, category] = splitPlural(key);
      if (category) plurals.set(base, [...(plurals.get(base) ?? []), category]);
    }
    for (const [base, have] of plurals) {
      const missing = categories.filter((c) => !have.includes(c));
      const extra = have.filter((c) => !categories.includes(c));
      if (missing.length || extra.length) {
        problems.push({ level: "error", namespace, key: base, message: `an English plural has exactly ${categories.map((c) => `_${c}`).join(" and ")}` });
      }
      if (base in flat) problems.push({ level: "error", namespace, key: base, message: "is both a plural and a message of its own" });
    }
  }
  return problems;
}

/** A folder's namespaces: each `<name>.json`, parsed, or the `Error` it didn't parse with. */
export function readNamespaces(dir) {
  const out = {};
  for (const file of readdirSync(dir).filter((f) => f.endsWith(".json")).sort()) {
    try {
      out[file.slice(0, -".json".length)] = JSON.parse(readFileSync(join(dir, file), "utf8"));
    } catch (error) {
      out[file.slice(0, -".json".length)] = error instanceof Error ? error : new Error(String(error));
    }
  }
  return out;
}

/**
 * Checks every language in `dir` (or those in `only`). Returns the problems, each with its
 * `locale`, and for each language other than English how many of English's messages it has.
 */
export function checkAll({ dir = LOCALES_DIR, locales = readLocales(), only = [], strict = false } = {}) {
  const problems = [];
  const coverage = {};
  const english = readNamespaces(join(dir, "en"));
  for (const p of checkEnglish(english)) problems.push({ locale: "en", ...p });

  const folders = existsSync(dir) ? readdirSync(dir, { withFileTypes: true }).filter((d) => d.isDirectory()).map((d) => d.name) : [];
  for (const folder of folders) {
    if (!locales.includes(folder)) {
      problems.push({ locale: folder, level: "error", message: `isn't one of the app's languages (src/i18n/locales.ts lists ${locales.join(", ")})` });
    }
  }
  for (const locale of locales.slice(1)) {
    if (only.length && !only.includes(locale)) continue;
    if (!folders.includes(locale)) {
      problems.push({ locale, level: "error", message: "has no folder. Make one with a {} file for each of English's namespaces" });
      continue;
    }
    const result = checkLocale(locale, english, readNamespaces(join(dir, locale)), { strict });
    for (const p of result.problems) problems.push({ locale, ...p });
    coverage[locale] = { messages: result.messages, translated: result.translated };
  }
  return { problems, coverage };
}

/** The command line: prints what it found and returns the exit code. */
export function main(argv = process.argv.slice(2), print = console.log) {
  const strict = argv.includes("--strict");
  const only = [];
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--locale") only.push(...(argv[++i] ?? "").split(",").filter(Boolean));
    else if (argv[i].startsWith("--locale=")) only.push(...argv[i].slice("--locale=".length).split(",").filter(Boolean));
  }
  const locales = readLocales();
  const unknown = only.filter((l) => !locales.includes(l) || l === "en");
  if (unknown.length) {
    print(`check-locales: ${unknown.join(", ")} isn't a language to check. Pick from ${locales.slice(1).join(", ")}`);
    return 2;
  }
  const { problems, coverage } = checkAll({ locales, only, strict });
  // A language none of whose namespaces is translated yet is one line, not one for each.
  const namespaces = readdirSync(join(LOCALES_DIR, "en")).filter((f) => f.endsWith(".json")).length;
  const untouched = new Set(
    Object.keys(coverage).filter((l) => problems.filter((p) => p.locale === l && p.untranslated).length === namespaces),
  );
  const lines = [];
  for (const p of problems) {
    if (p.untranslated && untouched.has(p.locale)) {
      if (!lines.some((l) => l.locale === p.locale && l.untranslated)) {
        lines.push({ ...p, namespace: undefined, message: `isn't translated yet: all ${coverage[p.locale].messages} of its messages show in English` });
      }
      continue;
    }
    lines.push(p);
  }
  const errors = lines.filter((p) => p.level === "error");
  const warnings = lines.filter((p) => p.level === "warning");
  for (const p of [...errors, ...warnings]) {
    const where = [p.locale, p.namespace && `${p.namespace}.json`, p.key].filter(Boolean).join(" ");
    print(`${p.level === "error" ? "error  " : "warning"} ${where}: ${p.message}`);
  }
  if (lines.length) print("");
  for (const [locale, { messages, translated }] of Object.entries(coverage)) {
    const share = messages ? Math.floor((translated / messages) * 100) : 100;
    print(`${locale.padEnd(6)} ${String(translated).padStart(5)} of ${messages} messages translated (${share}%)`);
  }
  const untranslated = problems.filter((p) => p.untranslated && p.level === "warning").length;
  print(
    errors.length
      ? `\n${errors.length} ${errors.length === 1 ? "problem" : "problems"} to fix. src/locales/TRANSLATORS.md has the rules.`
      : `\nNo problems${untranslated ? `, and ${untranslated} ${untranslated === 1 ? "namespace" : "namespaces"} still to translate` : ""}.`,
  );
  return errors.length ? 1 : 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = main();
}
