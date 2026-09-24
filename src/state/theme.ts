/** Light / dark theme, following the system unless the user picks one. */

import { useSyncExternalStore } from "react";

export type Theme = "light" | "dark";
export type ThemePref = Theme | "system";

export const THEME_KEY = "folderskin.theme";

export function loadThemePref(): ThemePref {
  try {
    const v = localStorage.getItem(THEME_KEY);
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

export function saveThemePref(pref: ThemePref): void {
  try {
    if (pref === "system") localStorage.removeItem(THEME_KEY);
    else localStorage.setItem(THEME_KEY, pref);
  } catch {
    /* storage unavailable: the choice lasts for the session */
  }
}

export function systemTheme(): Theme {
  return typeof matchMedia === "function" && matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function resolveTheme(pref: ThemePref): Theme {
  return pref === "system" ? systemTheme() : pref;
}

/** Next value when the user taps the switch: always the opposite of what they see. */
export function toggleTheme(current: Theme): Theme {
  return current === "dark" ? "light" : "dark";
}

export function applyTheme(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
}

/** The theme on screen now: the one applyTheme put on <html>. */
function shownTheme(): Theme {
  return document.documentElement.dataset.theme === "dark" ? "dark" : "light";
}

function watchTheme(changed: () => void): () => void {
  const observer = new MutationObserver(changed);
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  return () => observer.disconnect();
}

/** The theme on screen, for a part of the app that draws itself to match: it follows every change. */
export function useShownTheme(): Theme {
  return useSyncExternalStore(watchTheme, shownTheme, () => "light");
}
