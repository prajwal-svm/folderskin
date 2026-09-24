/**
 * Which folder skins go on: FolderSkin's own, as a Mac shows it, or the one Windows draws. Rust
 * keeps the choice (src-tauri/src/look.rs) and draws artwork skins on it; this mirrors it for the
 * parts drawn here: the empty folder in the folder panel and the folder new designs start on.
 * Every computer starts on the Mac's.
 */
import { useSyncExternalStore } from "react";
import type { FolderStyle } from "../composer/parts";
import { api } from "../lib/tauri";

let current: FolderStyle = "mac";
const listeners = new Set<() => void>();

function set(look: FolderStyle) {
  if (look === current) return;
  current = look;
  for (const l of listeners) l();
}

/** Asks Rust which folder it is, once the app starts. */
export function startLook(): Promise<void> {
  return api
    .folderLook()
    .then((look) => set(look === "windows" ? "windows" : "mac"))
    .catch(() => {
      // The Mac's, as every computer starts.
    });
}

export const getLook = (): FolderStyle => current;

/** Puts skins on the other folder from now on. Resolves once Rust has it, so thumbnails can be fetched again. */
export async function chooseLook(look: FolderStyle): Promise<void> {
  await api.setFolderLook(look);
  set(look);
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useLook(): FolderStyle {
  return useSyncExternalStore(subscribe, getLook, getLook);
}
