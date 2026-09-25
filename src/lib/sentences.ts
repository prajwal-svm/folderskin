/**
 * What the app's Rust side says, in the language on show. The Rust side writes its errors and its
 * progress as English sentences (docs/ARCHITECTURE.md, "Errors cross the boundary as plain
 * strings"), and the webview shows them. Every sentence in the `native` catalog is one of those,
 * word for word, with `{{name}}` for the parts that vary; a sentence that matches one is said again
 * from the catalog in the language on show, and anything else is shown as it came. In English the
 * sentence comes back exactly as it was. sentences.test.ts checks every catalog sentence is still
 * one the Rust side writes.
 */
import { flatten, PLACEHOLDER, type Vars } from "../i18n/core";
import { t, type MessageKey } from "../i18n";
import native from "../locales/en/native.json";
import { providerName } from "./providerNames";

type Matcher = { key: MessageKey; re: RegExp };

/** The parts of a sentence that are themselves sentences from the Rust side, said again in turn. */
const NESTED = new Set(["reason", "detail"]);

/** Parts that are a file's name, which has no colon: "b.png: it arrived damaged" is a name and a sentence. */
const FILE_NAMES = new Set(["file"]);

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/**
 * A regular expression for one English sentence: its words exactly, each `{{name}}` any text, and
 * the full stop at the end optional. The first letter's case doesn't matter, since the Rust side
 * writes some sentences with a capital and a stop (`sentence()` in ai/failure.rs).
 */
export function matcherFor(english: string): RegExp {
  const body = english.replace(/\.$/, "");
  let source = "";
  let at = 0;
  const seen = new Set<string>();
  for (const m of body.matchAll(PLACEHOLDER)) {
    source += escape(body.slice(at, m.index));
    const name = m[1];
    // A name used twice is the same text both times.
    source += seen.has(name) ? `\\k<${name}>` : FILE_NAMES.has(name) ? `(?<${name}>[^:]+?)` : `(?<${name}>.+?)`;
    seen.add(name);
    at = m.index + m[0].length;
  }
  source += escape(body.slice(at));
  const first = source.charAt(0);
  if (/[a-z]/i.test(first)) source = `[${first.toLowerCase()}${first.toUpperCase()}]${source.slice(1)}`;
  return new RegExp(`^${source}\\.?$`, "s");
}

let matchers: (Matcher & { english: string })[] | null = null;

/** Every sentence of the `native` catalog, most specific first: fixed words before open ones. */
function all(): (Matcher & { english: string })[] {
  if (matchers) return matchers;
  const english = flatten(native, "native");
  const fixed = (s: string) => s.replace(PLACEHOLDER, "").length;
  matchers = Object.entries(english)
    .sort(([, a], [, b]) => fixed(b) - fixed(a))
    .map(([key, value]) => ({ key: key as MessageKey, re: matcherFor(value), english: value }));
  return matchers;
}

const isUpper = (c: string) => c !== c.toLocaleLowerCase() && c === c.toLocaleUpperCase();

/**
 * The translation shaped like the sentence it came from, where the catalog's English and that
 * sentence differ only in a capital first letter or a full stop at the end: so English comes back
 * exactly as the Rust side wrote it.
 */
function shapedLike(said: string, text: string, english: string): string {
  let out = said;
  const first = text.charAt(0);
  if (first && english.charAt(0) !== first && english.charAt(0).toLocaleLowerCase() === first.toLocaleLowerCase()) {
    out = (isUpper(first) ? out.charAt(0).toLocaleUpperCase() : out.charAt(0).toLocaleLowerCase()) + out.slice(1);
  }
  if (english.endsWith(".") && !text.endsWith(".")) out = out.replace(/[.。]$/u, "");
  return out;
}

/** `message`, a sentence from the Rust side, in the language on show when it's one of the known ones. */
export function explain(message: string): string {
  const text = message.trim();
  if (!text) return message;
  for (const { key, re, english } of all()) {
    const m = re.exec(text);
    if (!m) continue;
    const vars: Vars = {};
    for (const [name, value] of Object.entries(m.groups ?? {})) {
      vars[name] = NESTED.has(name) ? explain(value) : name === "provider" ? providerName(value) : value;
    }
    return shapedLike(t(key, vars), text, english);
  }
  // "b.png: it isn't on github.com any more. Try Refresh": a file's name, then a known sentence.
  const labelled = /^(?<label>[^\s:]+): (?<rest>.+)$/s.exec(text);
  if (labelled?.groups) {
    const rest = explain(labelled.groups.rest);
    if (rest !== labelled.groups.rest) return `${labelled.groups.label}: ${rest}`;
  }
  return message;
}
