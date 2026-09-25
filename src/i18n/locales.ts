/**
 * The languages FolderSkin speaks, what each is called in itself, and which one a computer asks
 * for. Pure, so it's tested on its own and the check script (scripts/check-locales.mjs) can keep
 * the same list.
 */

/** Every language the app is translated into. English first: it's the one everything falls back to. */
export const LOCALES = ["en", "zh-CN", "ja", "ko", "fr", "es"] as const;

export type Locale = (typeof LOCALES)[number];

export const DEFAULT_LOCALE: Locale = "en";

/** Each language in its own words, as the language menu lists them. Never translated. */
export const LOCALE_NAMES: Record<Locale, string> = {
  en: "English",
  "zh-CN": "简体中文",
  ja: "日本語",
  ko: "한국어",
  fr: "Français",
  es: "Español",
};

/**
 * The locale `Intl` formats numbers, dates and lists in. The app's English is British
 * ("favourites", "licence"), so its dates and lists are too: "25 Sept 2026", "A, B and C".
 */
export const INTL_LOCALES: Record<Locale, string> = {
  en: "en-GB",
  "zh-CN": "zh-CN",
  ja: "ja",
  ko: "ko",
  fr: "fr",
  es: "es",
};

export function isLocale(value: unknown): value is Locale {
  return typeof value === "string" && (LOCALES as readonly string[]).includes(value);
}

/**
 * The app's language for one BCP 47 tag, or null when the app doesn't speak it. Any Chinese is
 * Simplified Chinese, the one Chinese there is; for the rest the region doesn't matter.
 */
export function matchLocale(tag: string): Locale | null {
  const lower = tag.trim().toLowerCase().replace(/_/g, "-");
  const lang = lower.split("-")[0];
  if (lang === "zh") return "zh-CN";
  return isLocale(lang) ? lang : null;
}

/**
 * The language a first launch opens in: the first of the computer's languages (the webview's
 * `navigator.languages`, most wanted first) that the app speaks, and English when it speaks none.
 */
export function detectLocale(languages: readonly string[] | undefined | null): Locale {
  for (const tag of languages ?? []) {
    const locale = matchLocale(tag);
    if (locale) return locale;
  }
  return DEFAULT_LOCALE;
}

/** The languages the webview says the computer wants, most wanted first. */
export function systemLanguages(): string[] {
  if (typeof navigator === "undefined") return [];
  if (Array.isArray(navigator.languages) && navigator.languages.length > 0) return [...navigator.languages];
  return navigator.language ? [navigator.language] : [];
}

/** FolderSkin's website, where the guides live. */
export const SITE_URL = "https://folderskin.app";

/** The website's own path for each language: English at the root, the rest under /<code>/. */
const SITE_PATHS: Record<Locale, string> = {
  en: "",
  "zh-CN": "/zh-cn",
  ja: "/ja",
  ko: "/ko",
  fr: "/fr",
  es: "/es",
};

/** A guide on folderskin.app in `locale`: `docsUrl("packs", "fr")` is https://folderskin.app/fr/docs/packs/. */
export function docsUrl(guide: string, locale: Locale): string {
  return `${SITE_URL}${SITE_PATHS[locale]}/docs/${guide.replace(/^\/+|\/+$/g, "")}/`;
}
