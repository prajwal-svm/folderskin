import { useSyncExternalStore } from "react";
import { api, errorMessage, type SavedPrompt } from "../lib/tauri";
import { styleById, type Look } from "../lib/styles";

/**
 * The prompts people save from the AI chat, which its "/" menu lists as Your prompts. The app keeps
 * them with its other data, as skills (prompts.rs); this holds the list for the session, read the
 * first time the chat opens, and changes it as prompts are saved, removed and put back.
 */

type State = {
  list: SavedPrompt[];
  /** Why the list couldn't be read, as a sentence; null when it was. */
  problem: string | null;
};

let state: State = { list: [], problem: null };
const listeners = new Set<() => void>();
let loaded: Promise<void> | null = null;

function set(next: Partial<State>) {
  state = { ...state, ...next };
  for (const l of listeners) l();
}

/** The words a saved prompt puts in the box. */
export const promptText = (p: SavedPrompt): string => p.idea?.trim() ?? "";

/** Whether a saved prompt adds a look of its own to its style: its own treatment, light, palette, lettering or keep-outs. */
export function hasOwnLook(p: SavedPrompt): boolean {
  return Boolean(p.treatment || p.light || p.palette?.length || p.keep_out?.length || p.lettering?.look);
}

/** The look a saved prompt paints in: its own, by its name; its style's; or none. */
export function promptLook(p: SavedPrompt): Look | null {
  if (hasOwnLook(p)) return { kind: "skill", id: p.id, name: p.name };
  const style = styleById(p.base_style);
  return style ? { kind: "style", id: style.id } : null;
}

/** Reads the saved prompts, once a session. */
export function loadPrompts(): Promise<void> {
  loaded ??= api
    .promptsList()
    .then((list) => set({ list, problem: null }))
    .catch((e) => set({ problem: errorMessage(e) }));
  return loaded;
}

/**
 * Saves `text` under `name`, in `look`, and says whether it replaced a prompt of that name.
 * Rejects with what went wrong, for the chat to say.
 */
export async function savePrompt(name: string, text: string, look: Look | null): Promise<{ saved: SavedPrompt; replaced: boolean }> {
  const saved = await api.promptSave(name, text, look?.id ?? null);
  const replaced = state.list.some((p) => p.id === saved.id);
  set({ list: replaced ? state.list.map((p) => (p.id === saved.id ? saved : p)) : [saved, ...state.list] });
  return { saved, replaced };
}

/**
 * Removes a saved prompt, and says where it was, for putting it back. It leaves the list at once
 * and comes back if removing it fails.
 */
export async function removePrompt(prompt: SavedPrompt): Promise<number> {
  const before = state.list;
  const at = before.findIndex((p) => p.id === prompt.id);
  set({ list: before.filter((p) => p.id !== prompt.id) });
  try {
    await api.promptDelete(prompt.id);
  } catch (e) {
    set({ list: before });
    throw e;
  }
  return at;
}

/** Puts a removed prompt back where it was, as it was. */
export async function restorePrompt(prompt: SavedPrompt, at: number): Promise<void> {
  const kept = await api.promptRestore(prompt, at < 0 ? null : at);
  if (state.list.some((p) => p.id === kept.id)) return;
  const list = [...state.list];
  list.splice(Math.max(0, Math.min(at, list.length)), 0, kept);
  set({ list });
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export function useSavedPrompts(): State {
  return useSyncExternalStore(subscribe, () => state);
}
