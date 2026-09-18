import { invoke } from "@tauri-apps/api/core";

/** A skin the gallery can show: built-in or imported from the user's picture. */
export type Skin = {
  id: string;
  name: string;
  collection: string;
  /** PNG data URL rendered by the Rust compositor (same pixels the app applies). */
  thumbnail: string;
  custom: boolean;
};

export type PathInfo = { kind: "folder" | "image" | "other"; name: string; path: string };

export type PlatformInfo = { os: string; browse_label: string; note: string };

/** Built-in skins plus the plain default folder rendered through the same compositor. */
export type SkinList = { skins: Skin[]; default_thumbnail: string };

/** Typed wrappers over the Tauri commands exposed by `src-tauri/src/commands.rs`. */
export const api = {
  listSkins: () => invoke<SkinList>("list_skins"),
  inspectPath: (path: string) => invoke<PathInfo>("inspect_path", { path }),
  importImage: (path: string) => invoke<Skin>("import_image", { path }),
  applySkin: (folder: string, skinId: string) => invoke<void>("apply_skin", { folder, skinId }),
  revertSkin: (folder: string) => invoke<void>("revert_skin", { folder }),
  platformInfo: () => invoke<PlatformInfo>("platform_info"),
};

/** Turns any thrown value from `invoke` into a sentence the drop zone can show. */
export function errorMessage(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return "something went wrong";
}
