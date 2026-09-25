/**
 * The language the app speaks: the one chosen from the sidebar's language menu, kept with the
 * other preferences (prefs.ts), or the computer's own until one is chosen. It's put on <html> as
 * `lang`, which the stylesheets read for the fonts Chinese, Japanese and Korean need, and handed to
 * the app itself for the menu bar and the parts macOS draws (src-tauri/src/language.rs).
 */
import { detectLocale, i18n, systemLanguages, t, type Locale, type MessageKey } from "../i18n";
import { api } from "../lib/tauri";
import { loadPrefs, setPrefs } from "./prefs";

/** The menu bar's items, by the name src-tauri/src/language.rs gives each. */
const MENU = [
  "about",
  "services",
  "hide",
  "hideOthers",
  "showAll",
  "quit",
  "file",
  "closeWindow",
  "edit",
  "undo",
  "redo",
  "cut",
  "copy",
  "paste",
  "selectAll",
  "view",
  "fullScreen",
  "window",
  "minimize",
  "zoom",
  "help",
] as const;

/** The menu bar's words in the language on show. */
export function menuWords(): Record<(typeof MENU)[number], string> {
  return Object.fromEntries(MENU.map((item) => [item, t(`menu.${item}` as MessageKey)])) as Record<(typeof MENU)[number], string>;
}

/** The language a launch opens in: the one chosen, or the computer's. */
export function startingLocale(): Locale {
  return loadPrefs().language ?? detectLocale(systemLanguages());
}

/** Tells the app the language, for its menu bar; `pin` when the person chose it. */
function tellApp(pin: boolean): void {
  api.setLanguage(i18n.locale(), menuWords(), pin).catch(() => {});
}

let watching = false;

/** Keeps <html lang> on the language on show. */
function watch(): void {
  if (watching || typeof document === "undefined") return;
  watching = true;
  const put = () => {
    document.documentElement.lang = i18n.locale();
  };
  i18n.subscribe(put);
  put();
}

/**
 * Loads the language a launch opens in, before anything is drawn, so nothing shows in English
 * first. It can't take long (the words are in the app), but English goes on regardless after
 * `wait` ms.
 */
export async function startLanguage(wait = 1500): Promise<void> {
  watch();
  const locale = startingLocale();
  if (locale !== i18n.locale()) {
    const ready = i18n.setLocale(locale).catch(() => {});
    await Promise.race([ready, new Promise<void>((resolve) => setTimeout(resolve, wait))]);
  }
  tellApp(false);
}

/** Puts the app in `locale`, now and at every launch after. */
export async function chooseLanguage(locale: Locale): Promise<void> {
  watch();
  setPrefs({ language: locale });
  await i18n.setLocale(locale);
  tellApp(true);
}
