import { Channel, invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri, mockApi } from "./devMock";

/** A skin in the library: a picture the user added, an AI result, or one from a community pack. All are saved on disk. */
export type Skin = {
  id: string;
  name: string;
  /** Always "yours": every skin is one the user added. */
  collection: string;
  /** PNG data URL rendered by the Rust compositor (same pixels the app applies). */
  thumbnail: string;
  custom: boolean;
  /** "artwork" is wrapped onto FolderSkin's folder; "folder" is a finished folder used as-is. */
  kind?: "artwork" | "folder";
  source?: "import" | "ai" | "community";
  /** Unix ms when the user added it. */
  created_at?: number | null;
  /** What the gallery filters it by, cleaned the way `cleanTag` does. */
  tags: string[];
  /** For a community skin, the id of the pack it came from. */
  pack?: string | null;
  /** AI results: the provider and model that made it, e.g. "OpenAI · GPT Image 2.5 Flare". */
  made_with?: string | null;
  /** AI results: the description it was made from. */
  idea?: string | null;
  /** Community skins: the pack's name, its author's GitHub name and its licence. */
  pack_name?: string | null;
  author?: string | null;
  license?: string | null;
};

/** One pack in the Community list. */
export type CommunityPack = {
  id: string;
  name: string;
  /** The author's GitHub user name. */
  author: string;
  license: string;
  tags: string[];
  count: number;
  /** The version on GitHub now; empty when the list doesn't say. */
  hash: string;
  /** True when its skins are in the library. */
  added: boolean;
  /** True when it was added and GitHub has a different version of it now. */
  update: boolean;
};

/** One skin of a pack being looked through before it's added. */
export type PackSkinPreview = {
  name: string;
  tags: string[];
  /** The skin as the folder it makes, as a data URL. */
  thumbnail: string;
};

/** How far adding a pack has got: pictures downloaded, then pictures saved. */
export type PackProgress = { stage: "download" | "save"; done: number; total: number };

/** What updating a pack changed. */
export type PackUpdate = {
  /** Old skins the new version doesn't have. */
  removed: string[];
  /** Every skin of the new version. */
  skins: Skin[];
};

/** What "Save as a pack" writes: some of the user's own skins, as a folder ready for GitHub. */
export type ExportPackRequest = {
  folder: string;
  name: string;
  author: string;
  license: string;
  tags: string[];
  skinIds: string[];
};

export type PathInfo = { kind: "folder" | "image" | "other"; name: string; path: string };

export type PlatformInfo = { os: string; browse_label: string; note: string };

/** The saved skins, newest first, plus the plain default folder rendered through the same compositor. */
export type SkinList = { skins: Skin[]; default_thumbnail: string };

export type AiModel = {
  id: string;
  label: string;
  native_alpha: boolean;
  accepts_reference: boolean;
  sizes: string[];
  price_hint: string;
};

export type AiProvider = {
  id: string;
  label: string;
  models: AiModel[];
  keys_url: string;
  docs_url: string;
  key_hint: string;
  /** True when a key for this provider is already saved (keys never come back to the webview). */
  has_key: boolean;
};

export type AiPreset = { id: string; label: string; idea: string };

export type AiCatalogue = { providers: AiProvider[]; presets: AiPreset[] };

export type AiGenerateRequest = {
  provider: string;
  model: string;
  /** The user's own words. */
  idea: string;
  /** "skin" wraps flat art onto our folder; "folder" uses the model's whole folder as the icon. */
  shape: "skin" | "folder";
  size: string | null;
  /** Optional reference picture already on disk. */
  reference_path: string | null;
  /** Tags for the result, such as the style the idea asks for. */
  tags: string[];
};

const tauriApi = {
  listSkins: () => invoke<SkinList>("list_skins"),
  inspectPath: (path: string) => invoke<PathInfo>("inspect_path", { path }),
  importImage: (path: string) => invoke<Skin>("import_image", { path }),
  applySkin: (folder: string, skinId: string) => invoke<void>("apply_skin", { folder, skinId }),
  revertSkin: (folder: string) => invoke<void>("revert_skin", { folder }),
  platformInfo: () => invoke<PlatformInfo>("platform_info"),
  /** The folder's current icon (data URL): the real OS icon where available. */
  folderIcon: (folder: string) => invoke<string>("folder_icon", { folder }),
  /** The folder the skins are saved in. */
  skinsFolder: () => invoke<string>("skins_folder"),
  /** Deletes one of the user's saved skins from disk. */
  deleteSkin: (skinId: string) => invoke<void>("delete_skin", { skinId }),
  /** Renames and retags one of the user's saved skins; resolves to both as saved. */
  editSkin: (skinId: string, name: string, tags: string[]) =>
    invoke<{ name: string; tags: string[] }>("edit_skin", { skinId, name, tags }),

  // ---- community packs (from the repository on GitHub) ----
  /** The packs on GitHub; `fresh` skips every cache, for Refresh. */
  communityPacks: (fresh = false) => invoke<CommunityPack[]>("community_packs", { fresh }),
  /** A pack's preview strip as a data URL, downloaded again when `fresh`. */
  communityPreview: (packId: string, fresh = false) => invoke<string>("community_preview", { packId, fresh }),
  /** Downloads a pack and saves all of its skins or none; resolves to them. `onProgress` hears how far it has got. */
  addPack: (packId: string, onProgress?: (progress: PackProgress) => void) =>
    invoke<Skin[]>("community_add", { packId, onProgress: new Channel<PackProgress>(onProgress) }),
  /** Every skin of a pack drawn as its folder, to look through before adding it. Saves nothing. */
  /** Downloads and draws a pack to look through, or shows it as drawn before when this version
   *  (`hash`) was looked at in the last week. */
  packSkins: (packId: string, hash: string) => invoke<PackSkinPreview[]>("community_pack_skins", { packId, hash }),
  /** Swaps an added pack's skins for the version on GitHub now. */
  updatePack: (packId: string) => invoke<PackUpdate>("community_update", { packId }),
  /** Deletes a pack's skins; resolves to their ids. */
  removePack: (packId: string) => invoke<string[]>("community_remove", { packId }),
  /** Adds a pack from a folder on this computer. */
  importPack: (path: string) => invoke<Skin[]>("import_pack", { path }),
  /** Writes skins as a pack folder inside `folder`; resolves to the folder it made. */
  exportPack: (req: ExportPackRequest) => invoke<string>("export_pack", { ...req }),
  // ---- first launch ----
  /** True until the first-launch onboarding has been finished on this computer. */
  onboardingNeeded: () => invoke<boolean>("onboarding_needed"),
  /** Remembers that the onboarding is done, so it never shows again. */
  finishOnboarding: () => invoke<void>("finish_onboarding"),

  /** Native window appearance; `null` follows the system. Keeps the macOS sidebar material in step with the app theme. */
  setWindowTheme: (theme: "light" | "dark" | null) => getCurrentWindow().setTheme(theme),

  // ---- AI assistant (bring your own key) ----
  aiCatalogue: () => invoke<AiCatalogue>("ai_catalogue"),
  aiSetKey: (provider: string, key: string) => invoke<void>("ai_set_key", { provider, key }),
  aiClearKey: (provider: string) => invoke<void>("ai_clear_key", { provider }),
  aiTestKey: (provider: string) => invoke<void>("ai_test_key", { provider }),
  aiGenerate: (req: AiGenerateRequest) => invoke<Skin>("ai_generate", { req }),
};

/** Typed wrappers over the Tauri commands exposed by `src-tauri/src/commands.rs`. */
export const api: typeof tauriApi = isTauri() ? tauriApi : mockApi;

/** Turns any thrown value from `invoke` into a sentence the drop zone can show. */
export function errorMessage(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return "something went wrong";
}
