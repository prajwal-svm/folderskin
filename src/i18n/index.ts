/**
 * The app's words in the language the person chose (Settings has none of this: the language menu
 * sits in the sidebar). English is bundled, every other language is a chunk of its own loaded the
 * first time it's shown, and anything a translation lacks comes from English.
 *
 * In a component, `const t = useT()` and `t("sidebar.addPhoto")`: the component draws again when
 * the language changes. Outside one, `t` from here reads the language on show when it's called, so
 * call it while drawing, never when a module loads. Keys are checked against the English catalogs
 * by the type checker. See src/locales/TRANSLATORS.md for the catalogs themselves.
 */
import { useSyncExternalStore } from "react";
import type { CatalogTree, PluralCategory, Vars } from "./core";
import { createI18n, type Loader, type Namespaces } from "./store";
import { docsUrl as docsUrlIn, LOCALES, type Locale } from "./locales";
import ai from "../locales/en/ai.json";
import common from "../locales/en/common.json";
import community from "../locales/en/community.json";
import native from "../locales/en/native.json";
import folder from "../locales/en/folder.json";
import library from "../locales/en/library.json";
import menu from "../locales/en/menu.json";
import onboarding from "../locales/en/onboarding.json";
import settings from "../locales/en/settings.json";
import share from "../locales/en/share.json";
import sidebar from "../locales/en/sidebar.json";
import updates from "../locales/en/updates.json";

export { DEFAULT_LOCALE, detectLocale, INTL_LOCALES, isLocale, LOCALE_NAMES, LOCALES, matchLocale, SITE_URL, systemLanguages, type Locale } from "./locales";
export type { Vars } from "./core";

/** English, in the app's first chunk: what every other language falls back to. */
const ENGLISH = { ai, common, community, folder, library, menu, native, onboarding, settings, share, sidebar, updates };

/**
 * English namespaces that arrive with the code that uses them, not in the first chunk: the
 * composer's, with the composer (i18n/composer.ts).
 */
type LazyEnglish = { composer: typeof import("../locales/en/composer.json") };

/** Every key a catalog has, "namespace.path.to.key", plural suffixes and all. */
type Leaves<T, P extends string> = T extends string
  ? P
  : { [K in keyof T & string]: Leaves<T[K], P extends "" ? K : `${P}.${K}`> }[keyof T & string];

/** A plural's key as code names it: `folder.count` for `folder.count_one` and `folder.count_other`. */
type Base<K extends string> = K extends `${infer B}_${PluralCategory}` ? B : K;

/** A key English has: `t` only takes these, so a typo or a key that was renamed fails to compile. */
export type MessageKey = Base<Leaves<typeof ENGLISH & LazyEnglish, "">>;

/** A message by key, placeholders filled: `t("folder.count", { count: 3 })`. */
export type TFunction = (key: MessageKey, vars?: Vars) => string;

/** Every other language's namespaces: `../locales/fr/sidebar.json` and so on, each loaded on demand. */
const FILES = import.meta.glob<CatalogTree>(["../locales/*/*.json", "!../locales/en/*.json"], { import: "default" });

/** Loads every namespace of `locale` at once (vite.config.ts puts them in one chunk per language). */
function loaderFor(locale: Locale): Loader {
  const prefix = `../locales/${locale}/`;
  const files = Object.entries(FILES).filter(([path]) => path.startsWith(prefix));
  return async () => {
    const namespaces: Namespaces = {};
    await Promise.all(
      files.map(async ([path, load]) => {
        namespaces[path.slice(prefix.length, -".json".length)] = await load();
      }),
    );
    return namespaces;
  };
}

const warned = new Set<string>();

export const i18n = createI18n({
  english: ENGLISH as Namespaces,
  loaders: Object.fromEntries(LOCALES.filter((l) => l !== "en").map((l) => [l, loaderFor(l)])),
  onMissing: (key) => {
    if (!import.meta.env.DEV || warned.has(key)) return;
    warned.add(key);
    console.warn(`i18n: no message for "${key}"`);
  },
});

/** The message for `key` in the language on show. Reads the language when it's called. */
export function t(key: MessageKey, vars?: Vars): string {
  return i18n.t()(key, vars);
}

const tNow = () => i18n.t() as TFunction;

/** `t` for a component, which then draws again whenever the language changes. */
export function useT(): TFunction {
  return useSyncExternalStore(i18n.subscribe, tNow, tNow);
}

/** The language on show. */
export function useLocale(): Locale {
  return useSyncExternalStore(i18n.subscribe, i18n.locale, i18n.locale);
}

/** The language on show, outside a component. */
export function getLocale(): Locale {
  return i18n.locale();
}

/** A guide on folderskin.app in the language on show: `docsUrl("packs")`. */
export function docsUrl(guide: string): string {
  return docsUrlIn(guide, i18n.locale());
}
