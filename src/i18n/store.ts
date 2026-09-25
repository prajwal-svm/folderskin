/**
 * The language on show and its messages. English is always here; another language is loaded the
 * first time it's asked for, then switched to all at once, so nothing shows half in one language.
 * No React: `index.ts` puts this in hooks.
 */
import { flatten, translate, type Catalog, type CatalogTree, type Vars } from "./core";
import { DEFAULT_LOCALE, INTL_LOCALES, type Locale } from "./locales";

/** A language's namespaces, each as its JSON: `{sidebar: {...}, library: {...}}`. */
export type Namespaces = Record<string, CatalogTree>;

/** Loads a language's namespaces. */
export type Loader = () => Promise<Namespaces>;

/** A message by key, placeholders filled from `vars`. */
export type Translate = (key: string, vars?: Vars) => string;

export type I18n = {
  /** The language on show. */
  locale: () => Locale;
  /** The message for `key` in the language on show. Changes identity whenever the language does. */
  t: () => Translate;
  /** Loads a language's messages, once; resolves when they're ready. A failed load can be tried again. */
  load: (locale: Locale) => Promise<void>;
  /** Loads `locale` if it needs to, then shows it. The last language asked for wins. */
  setLocale: (locale: Locale) => Promise<void>;
  /** Adds English namespaces that arrive with the code that shows them (the composer's). */
  addEnglish: (namespaces: Namespaces) => void;
  subscribe: (listener: () => void) => () => void;
};

/** Namespaces flattened into one catalog: `library.json`'s `empty.title` is `library.empty.title`. */
export function catalogOf(namespaces: Namespaces): Catalog {
  const out: Catalog = {};
  for (const [ns, tree] of Object.entries(namespaces)) flatten(tree, ns, out);
  return out;
}

export function createI18n({
  english,
  loaders,
  onMissing,
}: {
  /** English, bundled with the app: every other language falls back to it. */
  english: Namespaces;
  /** How to load every other language. */
  loaders: Partial<Record<Locale, Loader>>;
  /** Hears a key that no catalog has. */
  onMissing?: (key: string) => void;
}): I18n {
  const fallback = catalogOf(english);
  const catalogs = new Map<Locale, Catalog>([[DEFAULT_LOCALE, fallback]]);
  const loading = new Map<Locale, Promise<void>>();
  const listeners = new Set<() => void>();
  let locale: Locale = DEFAULT_LOCALE;
  let wanted: Locale = DEFAULT_LOCALE;

  const make = (at: Locale): Translate => {
    const lookup = { locale: at, intl: INTL_LOCALES[at], active: catalogs.get(at) ?? fallback, fallback };
    return (key, vars) => translate(lookup, key, vars, onMissing);
  };
  let t = make(locale);

  const notify = () => {
    for (const l of listeners) l();
  };

  const load = (at: Locale): Promise<void> => {
    if (catalogs.has(at)) return Promise.resolve();
    const loader = loaders[at];
    if (!loader) return Promise.reject(new Error(`no messages for ${at}`));
    let pending = loading.get(at);
    if (!pending) {
      pending = loader()
        .then((namespaces) => {
          catalogs.set(at, catalogOf(namespaces));
        })
        .finally(() => loading.delete(at));
      loading.set(at, pending);
    }
    return pending;
  };

  return {
    locale: () => locale,
    t: () => t,
    load,
    async setLocale(next) {
      wanted = next;
      await load(next);
      // Another language was asked for while this one loaded: that one wins.
      if (wanted !== next || locale === next) return;
      locale = next;
      t = make(next);
      notify();
    },
    addEnglish(namespaces) {
      // Every lookup reads English as it is when it looks, so nothing needs to draw again: the
      // code that shows these words arrives with them.
      Object.assign(fallback, catalogOf(namespaces));
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}
