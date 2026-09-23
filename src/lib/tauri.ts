import { Channel, invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri, mockApi } from "./devMock";
import { frame } from "../composer/body";
import type { Parts } from "../composer/parts";
import type { Subfolders, TreeProgress, TreeRunResult } from "./tree";
import type { AiEvent } from "../state/chats";

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
  source?: "import" | "ai" | "community" | "composer";
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

/** Who is signed in to GitHub. */
export type GithubAccount = { login: string; name: string | null; avatar_url: string };

/** What someone types into github.com/login/device to let FolderSkin act for them. */
export type DeviceCode = { user_code: string; verification_uri: string; expires_in: number };

/** How far publishing has got. */
export type PublishProgress =
  | { stage: "checking" }
  | { stage: "forking" }
  | { stage: "branching" }
  | { stage: "uploading"; done: number; total: number }
  | { stage: "opening" };

/** The pull request that was opened. */
export type Published = { url: string; number: number; forked: boolean };

/** A pack on its way to GitHub. The author isn't here: whoever is signed in is who it is
 *  credited to, which GitHub tells us and nobody can mistype. */
export type PackToPublish = { name: string; license: string; tags: string[]; skinIds: string[]; notes: string; termsVersion: number };

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

/** A folder's icon as it looks now (a data URL), and whether it's a custom one a revert would take off. */
export type FolderIcon = { url: string; custom: boolean };

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
  /** "local" runs on this computer and needs no key; the rest use the user's own key. */
  kind?: "key" | "local";
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
  /** Every reference picture, for the models that take more than one. */
  reference_paths?: string[];
  /** Tags for the result, such as the style the idea asks for. */
  tags: string[];
  /** Names this run, so aiCancel can stop it. */
  job?: string;
};

/** Whether pictures can be made on this computer, and what it takes (the local engine). */
export type LocalStatus = {
  /** The runtime and a model are here and checked. */
  ready: boolean;
  /** How it runs here: CUDA, Vulkan, Metal, MLX or CPU. */
  backend: string;
  /** What it runs on, as people know it ("NVIDIA GeForce RTX 3050 Ti, 4 GB"). */
  device: string;
  /** What's still to download before it's ready; 0 once it is. */
  download_bytes: number;
  /** Roughly how long one picture takes here, when known. */
  seconds_per_image: number | null;
  /** Where the runtime and models are kept. */
  home: string;
  /** Anything worth knowing before setting up, such as too little memory for the best model. */
  note: string | null;
};

/** A saved conversation with the assistant, as the history lists it (chats.rs). */
export type ChatSummaryDto = { id: string; title: string; created: number; updated: number; turns: number; cover: string | null };

/** A reference picture kept for a chat: a shrunk copy in the chat's own folder, and a thumbnail. */
export type ChatRefDto = { id: string; name: string; path: string; thumb: string };

/** The folder template split into the layers the composer draws a design between, as PNG data URLs. */
export type ComposerTemplate = {
  /** Edge of every layer, in pixels. */
  size: number;
  back: string;
  front: string;
  middle: string;
  top: string;
  outline: string;
  parts: Parts;
};

/** A picture for a composer layer: a file the user picked, or one of their skins. */
export type ComposerImage = { url: string; width: number; height: number; name: string; alpha: boolean };

/** What saving a design sends with its picture. `replaces` is the design being changed, if any. */
export type ComposerSaveHeader = {
  name: string;
  tags: string[];
  shape: "folder" | "free";
  design: unknown;
  replaces: string | null;
};

/** A saved design, and the id of the one it replaced. */
export type ComposerSaved = { skin: Skin; replaced: string | null };

/** How far an icon pack's download has got, in bytes. */
export type IconPackProgress = { done: number; total: number };
export type InstalledIconPack = { id: string; sha256: string; bytes: number };

const tauriApi = {
  listSkins: () => invoke<SkinList>("list_skins"),

  // ---- publishing a pack to GitHub ----
  /** Who is signed in, or null. A sign-in GitHub no longer accepts counts as none. */
  githubAccount: () => invoke<GithubAccount | null>("github_account"),
  /** Asks for a code to show. `githubWait` then resolves when it has been approved. */
  githubConnect: () => invoke<DeviceCode>("github_connect"),
  githubWait: () => invoke<GithubAccount>("github_wait"),
  githubCancel: () => invoke<void>("github_cancel"),
  githubSignOut: () => invoke<void>("github_sign_out"),
  /** Opens a pull request that adds the pack to the community repository. */
  publishPack: (pack: PackToPublish, onProgress: (p: PublishProgress) => void) =>
    invoke<Published>("publish_pack", { pack, onProgress: new Channel<PublishProgress>(onProgress) }),
  inspectPath: (path: string) => invoke<PathInfo>("inspect_path", { path }),
  importImage: (path: string) => invoke<Skin>("import_image", { path }),
  applySkin: (folder: string, skinId: string) => invoke<void>("apply_skin", { folder, skinId }),
  revertSkin: (folder: string) => invoke<void>("revert_skin", { folder }),
  platformInfo: () => invoke<PlatformInfo>("platform_info"),

  // ---- a folder and every folder inside it ----
  /** How many folders are inside `folder` (hidden ones, packages and links aside), up to 5,000. */
  subfolderCount: (folder: string) => invoke<Subfolders>("subfolder_count", { folder }),
  /** The disk space one folder's copy of this skin's icon takes. */
  treeBytes: (skinId: string) => invoke<number>("tree_bytes", { skinId }),
  /** Applies a skin to `folder` and every folder inside it, or to `only` those; progress on `onProgress`. */
  applySkinTree: (folder: string, skinId: string, only: string[] | null, onProgress: (p: TreeProgress) => void) =>
    invoke<TreeRunResult>("apply_skin_tree", { folder, skinId, only, onProgress: new Channel<TreeProgress>(onProgress) }),
  /** Puts the default icon back on `only` those folders, or on every folder in the tree that has an icon of its own. */
  revertSkinTree: (folder: string, only: string[] | null, onProgress: (p: TreeProgress) => void) =>
    invoke<TreeRunResult>("revert_skin_tree", { folder, only, onProgress: new Channel<TreeProgress>(onProgress) }),
  /** Stops a run over a tree after the folder it's on. */
  stopTreeRun: () => invoke<void>("stop_tree_run"),
  /** The folder's current icon (the real OS icon where available), and whether it's a custom one. */
  folderIcon: (folder: string) => invoke<FolderIcon>("folder_icon", { folder }),
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
  /** Makes a picture and saves it as a skin, telling `onEvent` how it's going as it goes. */
  aiGenerate: (req: AiGenerateRequest, onEvent: (event: AiEvent) => void = () => {}) =>
    invoke<Skin>("ai_generate", { req, onEvent: new Channel<AiEvent>(onEvent) }),
  /** Stops the run named `job`: its request is dropped, or its local model stopped. */
  aiCancel: (job: string) => invoke<void>("ai_cancel", { job }),
  /** Whether pictures can be made on this computer, and what setting that up takes. */
  aiLocalStatus: () => invoke<LocalStatus>("ai_local_status"),
  /** Downloads and checks the runtime and model for this computer, telling `onEvent` as it goes. */
  aiLocalSetup: (onEvent: (event: AiEvent) => void) => invoke<LocalStatus>("ai_local_setup", { onEvent: new Channel<AiEvent>(onEvent) }),

  // ---- the assistant's saved chats (chats.rs) ----
  chatsList: () => invoke<ChatSummaryDto[]>("chats_list"),
  chatRead: (id: string) => invoke<unknown>("chat_read", { id }),
  chatSave: (chat: unknown) => invoke<ChatSummaryDto>("chat_save", { chat }),
  chatDelete: (id: string) => invoke<void>("chat_delete", { id }),
  chatKeepReference: (id: string, path: string) => invoke<ChatRefDto>("chat_keep_reference", { id, path }),

  // ---- icon packs ----
  /** Downloads a pack from its release and keeps it, if it is exactly `bytes` long with SHA-256 `sha256`. */
  iconPackDownload: (id: string, release: string, sha256: string, bytes: number, onProgress: (p: IconPackProgress) => void) =>
    invoke<void>("icon_pack_download", { id, release, sha256, bytes, onProgress: new Channel<IconPackProgress>(onProgress) }),
  /** The packs downloaded on this computer. */
  iconPacksInstalled: () => invoke<InstalledIconPack[]>("icon_packs_installed"),
  /** A downloaded pack's JSON. */
  iconPackRead: (id: string) => invoke<string>("icon_pack_read", { id }),
  /** Forgets a downloaded pack; designs using its icons keep them. */
  iconPackRemove: (id: string) => invoke<void>("icon_pack_remove", { id }),

  // ---- the composer ----
  /** The folder template's layers, rendered once by the Rust compositor. */
  composerTemplate: () => invoke<ComposerTemplate>("composer_template"),
  /** Saves a design (its full-size picture and its document) as a skin, or changes one saved before. */
  composerSave: (header: ComposerSaveHeader, png: Uint8Array) => invoke<ComposerSaved>("composer_save", frame(header, png)),
  /** The design as the icon at each of `sizes`, as data URLs, drawn by the compositor. */
  composerPreview: (shape: "folder" | "free", sizes: number[], png: Uint8Array) =>
    invoke<string[]>("composer_preview", frame({ shape, sizes }, png)),
  /** A picture file, read (and shrunk) for a picture layer. */
  composerImage: (path: string) => invoke<ComposerImage>("composer_image", { path }),
  /** A saved skin's own picture, for a picture layer or a remix. */
  composerSkinImage: (skinId: string) => invoke<ComposerImage>("composer_skin_image", { skinId }),
  /** The document of a saved design, to edit it again; null for a skin that wasn't made in the composer. */
  composerDesign: (skinId: string) => invoke<unknown>("composer_design", { skinId }),
};

/**
 * Typed wrappers over the Tauri commands exposed by `src-tauri/src/commands.rs`. `pnpm dev` in a
 * plain browser gets the stand-ins instead; a build leaves them out, since it only runs in the app.
 */
export const api: typeof tauriApi = import.meta.env.DEV && !isTauri() ? mockApi : tauriApi;

/** Turns any thrown value from `invoke` into a sentence the drop zone can show. */
export function errorMessage(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  // A structured error (the AI commands' {code, message}) says itself in its message.
  if (err && typeof err === "object" && typeof (err as { message?: unknown }).message === "string") return (err as { message: string }).message;
  return "something went wrong";
}
