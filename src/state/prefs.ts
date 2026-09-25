/**
 * How the app looks beyond light and dark (state/theme.ts): its accent colour, how much it moves,
 * and the language it speaks. All are kept on this computer. The first two are set on <html> as
 * data attributes, which tokens.css and base.css read, so every part of the app follows without
 * being told; the language is state/language.ts's to put on show.
 */
import { useSyncExternalStore } from "react";
import { isLocale, type Locale } from "../i18n/locales";

/** The accents Settings offers, each drawn by its --swatch-<id> in tokens.css. */
export const ACCENTS = [
  { id: "blue", label: "Blue" },
  { id: "purple", label: "Purple" },
  { id: "pink", label: "Pink" },
  { id: "orange", label: "Orange" },
  { id: "green", label: "Green" },
  /** Black on a light window, white on a dark one. */
  { id: "mono", label: "Black and white" },
] as const;

export type Accent = (typeof ACCENTS)[number]["id"];

/** "system" follows the computer's own reduce-motion setting; "reduced" is always still. */
export type Motion = "system" | "reduced";

/**
 * The language chosen from the sidebar's language menu, or null while none has been: the app then
 * follows the computer's.
 */
export type Prefs = { accent: Accent; motion: Motion; language: Locale | null };

export const PREFS_KEY = "folderskin.prefs";
const DEFAULTS: Prefs = { accent: "blue", motion: "system", language: null };

/** What was saved, with anything unknown or missing back at its default. */
export function readPrefs(raw: unknown): Prefs {
  if (typeof raw !== "object" || raw === null) return { ...DEFAULTS };
  const v = raw as Record<string, unknown>;
  // Graphite became black and white.
  const saved = v.accent === "graphite" ? "mono" : v.accent;
  const accent = ACCENTS.some((a) => a.id === saved) ? (saved as Accent) : DEFAULTS.accent;
  const motion = v.motion === "reduced" ? "reduced" : "system";
  const language = isLocale(v.language) ? v.language : null;
  return { accent, motion, language };
}

export function loadPrefs(): Prefs {
  try {
    return readPrefs(JSON.parse(localStorage.getItem(PREFS_KEY) ?? "null"));
  } catch {
    return { ...DEFAULTS };
  }
}

/** Sets the attributes the stylesheets read. The defaults are no attribute at all. */
export function applyPrefs(prefs: Prefs, root: HTMLElement = document.documentElement): void {
  if (prefs.accent === DEFAULTS.accent) delete root.dataset.accent;
  else root.dataset.accent = prefs.accent;
  if (prefs.motion === "reduced") root.dataset.motion = "reduced";
  else delete root.dataset.motion;
}

let current: Prefs | null = null;
const listeners = new Set<() => void>();

function snapshot(): Prefs {
  return (current ??= loadPrefs());
}

/** Changes some of the preferences, keeps them, and puts them on the page. */
export function setPrefs(patch: Partial<Prefs>): void {
  current = { ...snapshot(), ...patch };
  try {
    localStorage.setItem(PREFS_KEY, JSON.stringify(current));
  } catch {
    /* nowhere to keep them: they last until the app closes */
  }
  applyPrefs(current);
  for (const l of listeners) l();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function usePrefs(): Prefs {
  return useSyncExternalStore(subscribe, snapshot, snapshot);
}

/**
 * Whether what moves by script should keep still: Motion is Reduced here, or the computer asks
 * for less motion. The stylesheets and the animated icons read the same two things themselves.
 */
export function reducesMotion(): boolean {
  if (snapshot().motion === "reduced") return true;
  return typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
}
