/** Favourite skins, persisted per user in localStorage (a per-viewer convenience only). */

export type KeyValueStore = {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
};

export const FAVORITES_KEY = "folderskin.favorites";

function safeStore(): KeyValueStore | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function loadFavorites(store: KeyValueStore | null = safeStore()): string[] {
  if (!store) return [];
  try {
    const raw = store.getItem(FAVORITES_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === "string") : [];
  } catch {
    return [];
  }
}

export function saveFavorites(ids: string[], store: KeyValueStore | null = safeStore()): void {
  if (!store) return;
  try {
    store.setItem(FAVORITES_KEY, JSON.stringify(ids));
  } catch {
    /* storage may be unavailable; favourites are a convenience */
  }
}

/** Returns the new list with `id` added or removed. */
export function toggleFavorite(ids: string[], id: string): string[] {
  return ids.includes(id) ? ids.filter((x) => x !== id) : [...ids, id];
}
