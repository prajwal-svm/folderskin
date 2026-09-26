import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri, mockApi } from "./devMock";
import { frame } from "../composer/body";
import type { FolderStyle, Parts } from "../composer/parts";
import type { SubfolderCount, SubfolderCounts, SubfolderList, TreeRunEvent } from "./tree";
import { askToNotify, notify, windowFocused } from "./notify";
import type { AiEvent } from "../state/chats";
import { t } from "../i18n";
import { explain } from "./sentences";

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
  /** Community skins: the pack's name, who it's credited to and its licence. */
  pack_name?: string | null;
  author?: string | null;
  license?: string | null;
};

/** One pack in the Community list. */
export type CommunityPack = {
  id: string;
  name: string;
  /** The name the pack is credited to. */
  author: string;
  license: string;
  tags: string[];
  count: number;
  /** What adding it downloads, in bytes; 0 when the list doesn't say. */
  bytes: number;
  /** The version published now; empty when the list doesn't say. */
  hash: string;
  /** The address of its preview strip: its first few skins as folders, side by side. */
  preview: string;
  /** True when its skins are in the library. */
  added: boolean;
  /** True when it was added and a different version of it is published now. */
  update: boolean;
  /** True for a pack the maintainer marks as official (`official.json` in folderskin-community). */
  official: boolean;
};

/** One skin of a pack being looked through before it's added. */
export type PackSkinPreview = {
  name: string;
  tags: string[];
  /** The skin as the folder it makes: a data URL, or the address of its thumbnail. */
  thumbnail: string;
};

/** The orders the Community list comes in. "best" is the best match for the words typed, or,
 *  with nothing typed, the featured packs and then the newest. */
export type CommunitySort = "best" | "newest" | "name" | "skins";

/** What to search the community packs for. */
export type CommunityQuery = { q: string; tag: string; sort: CommunitySort; offset: number; limit: number };

/** A skin whose name matches a search, with the pack it's in. */
export type SkinHit = { pack: string; pack_name: string; name: string; index: number; thumbnail: string };

/** One page of a search, and what goes with it. */
export type CommunitySearch = {
  /** Packs matching the words and the tag. */
  total: number;
  /** Packs matching the words, whatever their tag: what "All" counts. */
  all: number;
  packs: CommunityPack[];
  skins: SkinHit[];
  /** The packs `skins` are in, so one can be opened wherever it is in the list. */
  hit_packs: CommunityPack[];
  /** The tags of the packs the words match, most used first. */
  facets: { tag: string; count: number }[];
  /** Why these are the packs from the last visit rather than the ones published now, as the
   *  start of a sentence ("you're offline"); null when they are the ones published now. */
  last_visit: string | null;
  /** Which catalog answered: a page from a newer one than the rest of the list says so. */
  generation: string;
};

/** What the first launch offers: its packs, and where any old id moved among them. */
export type FirstPacks = {
  packs: CommunityPack[];
  /** Each old id that now leads to one of `packs`, to the id it leads to: packs can move to a new
   *  id, and the onboarding picks one by the id it had first. */
  moved: Record<string, string>;
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

/** What "Save a folder" writes: some of the user's own skins, as a pack folder anyone can add from. */
export type ExportPackRequest = {
  folder: string;
  name: string;
  author: string;
  license: string;
  tags: string[];
  skinIds: string[];
};

/** How far making a pack's pictures ready has got: `done` of `total` are. Several are made at once. */
export type MakeProgress = { done: number; total: number };

/** A picture made smaller than 1024 px so it fits the 1.5 MB a pack's picture can be, still lossless. */
export type ScaledPicture = { name: string; side: number };

/** A pack saved as a folder: where it is, and the pictures made smaller to fit. */
export type ExportedPack = { folder: string; scaled: ScaledPicture[] };

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

/** The job name setting this computer up runs under, for `aiCancel` (ai/jobs.rs `LOCAL_SETUP`). */
export const LOCAL_SETUP_JOB = "local-setup";

/** Whether pictures can be made on this computer, and what it takes (the local engine). */
export type LocalStatus = {
  /** The runtime and the model are here, and the runtime starts. */
  ready: boolean;
  /** Setting up has something to install here; false on a computer the runtime has no build for (an Intel Mac, ARM64 Linux), where `note` says what to do instead. */
  can_set_up: boolean;
  /** A setup is under way, perhaps started before this panel opened: `aiLocalSetup` joins it. */
  setting_up: boolean;
  /** How it runs here: CUDA, Vulkan, Metal, MLX or CPU. */
  backend: string;
  /** What it runs on, as people know it ("NVIDIA GeForce RTX 3050 Ti, 4 GB"). */
  device: string;
  /** What's still to download before it's ready (the runtime's build and the model), less what's here; 0 once it is. */
  download_bytes: number;
  /** The runtime setting up installs besides that, while it isn't installed: "mflux" on Apple Silicon, installed with a uv and a Python of its own, whose packages `download_bytes` can't count. */
  installs: string | null;
  /** What setup's files take on disk now, partial downloads included: what removing the model gives back. */
  kept_bytes: number;
  /** Of those, the model files an earlier setup left that the model doesn't use now (the other tier's, or Z-Image Turbo's). */
  unused_bytes: number;
  /** The model it paints with ("FLUX.2 [klein] 4B"), how finely ("4-bit") and what its files come to here. */
  model: string;
  quality: string;
  model_bytes: number;
  /** Free space on the disk the model goes on (null when the system doesn't say), and what setting up wants free: with less, it won't start. */
  free_bytes: number | null;
  wanted_bytes: number;
  /** How long the last picture painted here took, once one has been. */
  seconds_per_image: number | null;
  /** Where the runtime and models are kept. */
  home: string;
  /** Anything worth knowing before setting up, such as too little memory for the best model. */
  note: string | null;
};

/** A saved conversation with the assistant, as the history lists it (chats.rs). */
export type ChatSummaryDto = { id: string; title: string; created: number; updated: number; turns: number; pictures: number; cover: string | null };

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
  style: FolderStyle;
  design: unknown;
  replaces: string | null;
};

/** A saved design, and the id of the one it replaced. */
export type ComposerSaved = { skin: Skin; replaced: string | null };

/** How far an icon pack's download has got, in bytes. */
export type IconPackProgress = { done: number; total: number };
export type InstalledIconPack = { id: string; sha256: string; bytes: number };

/** Whether sharing can be used here, and who this computer is to the service. */
export type ShareStatus = {
  /** False when this build has no service, it can't be reached, it's paused, or this computer can't share. */
  available: boolean;
  /** Why not, as a sentence. */
  reason: string | null;
  verified: boolean;
  /** The name packs from this computer are credited to, once verified. */
  handle: string | null;
  /** Whether this computer has a sharing key yet, which is what a recovery file saves. */
  has_key: boolean;
  /** The version of the pack terms packs are sent under now, as the service says; null while
   *  sharing can't be used. It's what a pack records the person agreed to. */
  terms_version: number | null;
};

/** A pack on its way to the review queue. The author isn't here: it's the name this computer was verified under. */
export type PackToShare = {
  name: string;
  license: string;
  tags: string[];
  skinIds: string[];
  notes: string;
  /** Where the pictures came from: own, ai, mixed or licensed. */
  source: string;
  /** The version of the pack terms they agreed to: {@link ShareStatus}'s `terms_version`. */
  termsVersion: number;
};

/** How far sending a pack for review has got. `encoding` is the pictures being made lossless WebP,
 *  several at once. `waiting` is a pause before a request that failed in a way that may pass is
 *  tried again; the stage it was part of comes again after it. */
export type ShareProgress =
  | { stage: "preparing" }
  | { stage: "encoding"; done: number; total: number }
  | { stage: "checking" }
  | { stage: "uploading"; done: number; total: number }
  | { stage: "finishing" }
  | { stage: "waiting"; seconds: number };

/** A pack in the review queue, and the pictures made smaller to fit. */
export type SharedPack = { submission_id: string; name: string; pictures: number; scaled: ScaledPicture[] };

/** One of this computer's packs, and where it is. */
export type MySubmission = {
  id: string;
  name: string;
  status: "uploading" | "in_review" | "approved" | "rejected" | "withdrawn" | "taken_down" | "expired";
  pictures: number;
  license: string;
  /** Unix seconds. */
  created_at: number;
  decided_at: number | null;
  /** The pack's folder name once approved. */
  pack_id: string | null;
  /** Whether it is in the community packs already, from where only the maintainer can take it out. */
  pulled: boolean;
  /** Why it was turned down or taken down: a rule in the pack terms, and a sentence. */
  reasons: { code: string; term: number; message: string }[];
  /** The maintainer's own words, when they left some. */
  note: string;
};

/**
 * Which folders inside one a run takes: every folder inside as `all` says, unless a rule on it
 * or a folder it's in says otherwise (lib/folderChoice.ts, `ChoiceDto` in src-tauri/src/tree.rs).
 */
export type RunChoice = { all: boolean; rules: { path: string; on: boolean }[] };

/** Hears `event` from the app until the returned function is called, even if that's before listening has begun. */
function hear<T>(event: string, listener: (payload: T) => void): () => void {
  let stop: (() => void) | null = null;
  let stopped = false;
  listen<T>(event, (e) => listener(e.payload))
    .then((unlisten) => {
      if (stopped) unlisten();
      else stop = unlisten;
    })
    .catch(() => {});
  return () => {
    stopped = true;
    stop?.();
  };
}

const tauriApi = {
  listSkins: () => invoke<SkinList>("list_skins"),
  /** Which folder artwork skins are drawn and applied on: "mac" unless the user chose Windows'. */
  folderLook: () => invoke<FolderStyle>("folder_look"),
  setFolderLook: (look: FolderStyle) => invoke<void>("set_folder_look", { look }),

  inspectPath: (path: string) => invoke<PathInfo>("inspect_path", { path }),
  importImage: (path: string) => invoke<Skin>("import_image", { path }),
  applySkin: (folder: string, skinId: string) => invoke<void>("apply_skin", { folder, skinId }),
  revertSkin: (folder: string) => invoke<void>("revert_skin", { folder }),
  platformInfo: () => invoke<PlatformInfo>("platform_info"),

  // ---- a folder and every folder inside it ----
  /**
   * Starts counting the folders inside `folder` (hidden ones, packages, links and other volumes
   * aside) in the background, however many, and stops counting the folder before. Answers with
   * the count once it's done, or after a moment, and `onSubfolderCount` hears the rest.
   */
  subfolderCount: (folder: string) => invoke<SubfolderCount>("subfolder_count", { folder }),
  /** How far the count has got inside `folder`, and inside each of `paths`, folders in it. */
  subfolderCounts: (folder: string, paths: string[]) => invoke<SubfolderCounts>("subfolder_counts", { folder, paths }),
  /** Hears every count grow. Returns what stops hearing it. */
  onSubfolderCount: (listener: (count: SubfolderCount) => void) => hear<SubfolderCount>("subfolder-count", listener),
  /** The folders directly inside `folder`, for one column of "Choose subfolders". */
  subfolderList: (folder: string) => invoke<SubfolderList>("subfolder_list", { folder }),
  /** The disk space one folder's copy of this skin's icon takes. */
  treeBytes: (skinId: string) => invoke<number>("tree_bytes", { skinId }),
  /**
   * Starts applying a skin to `folder` and the folders inside it `choice` takes (every one without
   * one), in the background. `onTreeRun` hears how it goes.
   */
  startTreeApply: (folder: string, skinId: string, choice: RunChoice | null) => invoke<TreeRunEvent>("start_tree_apply", { folder, skinId, choice }),
  /**
   * Starts taking the custom icons off `folder` and the folders inside it `choice` takes, in the
   * background. `skipPlain` leaves the ones without an icon of their own alone.
   */
  startTreeRevert: (folder: string, choice: RunChoice | null, skipPlain: boolean) =>
    invoke<TreeRunEvent>("start_tree_revert", { folder, choice, skipPlain }),
  /** Stops run `id` after the folder it's on. */
  stopTreeRun: (id: number) => invoke<void>("stop_tree_run", { id }),
  /** Carries stopped run `id` on with the folders it didn't reach. */
  carryOnTreeRun: (id: number) => invoke<TreeRunEvent>("carry_on_tree_run", { id }),
  /** Tries the folders run `id` couldn't change once more. */
  retryTreeRun: (id: number) => invoke<TreeRunEvent>("retry_tree_run", { id }),
  /** Takes off exactly what apply run `id` put on. */
  undoTreeRun: (id: number) => invoke<TreeRunEvent>("undo_tree_run", { id }),
  /** Forgets run `id` once it has ended. */
  dismissTreeRun: (id: number) => invoke<void>("dismiss_tree_run", { id }),
  /** The latest run, going or ended. */
  treeRun: () => invoke<TreeRunEvent>("tree_run"),
  /** Hears the latest run change. Returns what stops hearing it. */
  onTreeRun: (listener: (event: TreeRunEvent) => void) => hear<TreeRunEvent>("tree-run", listener),
  /** Whether FolderSkin's window is the one in front. */
  windowFocused,
  /** Whether notifications may be shown, asking the first time. */
  askToNotify,
  /** A system notification, if they may be shown. */
  notify,
  /** The folder's current icon (the real OS icon where available), and whether it's a custom one. */
  folderIcon: (folder: string) => invoke<FolderIcon>("folder_icon", { folder }),
  /** The folder the skins are saved in. */
  skinsFolder: () => invoke<string>("skins_folder"),
  /** Deletes one of the user's saved skins from disk. */
  deleteSkin: (skinId: string) => invoke<void>("delete_skin", { skinId }),
  /** Renames and retags one of the user's saved skins; resolves to both as saved. */
  editSkin: (skinId: string, name: string, tags: string[]) =>
    invoke<{ name: string; tags: string[] }>("edit_skin", { skinId, name, tags }),

  // ---- community packs (from packs.folderskin.app, or the repository on GitHub) ----
  /** The packs the first launch offers: the featured ones, or the first few. `fresh` skips every cache. */
  communityPacks: (fresh = false) => invoke<FirstPacks>("community_packs", { fresh }),
  /** One page of the packs matching a search, searched on this computer once the catalog is in. */
  communitySearch: (query: CommunityQuery) => invoke<CommunitySearch>("community_search", { ...query }),
  /** Asks for the packs again past every cache; resolves to how many of the library's packs have an update. */
  communityRefresh: () => invoke<{ updates: number; packs: number }>("community_refresh"),
  /** The packs in the library now, by id, with the version each was added at (null when it
   *  was added before FolderSkin kept one). Nothing is downloaded. */
  communityInstalled: () => invoke<Record<string, string | null>>("community_installed"),
  /** One pack by its id, or an id it had before it moved, as the list shows it; null when no pack
   *  has or had that id. A pack the list doesn't have is looked for again past every cache, in
   *  case it was published since. */
  communityPack: (packId: string) => invoke<CommunityPack | null>("community_pack", { packId }),
  /** The pack a folderskin://install link asked for, once; null when none is waiting. */
  takeInstallLink: () => invoke<string | null>("install_link_take"),
  /** Downloads a pack and saves all of its skins or none; resolves to them. `onProgress` hears how far it has got. */
  addPack: (packId: string, onProgress?: (progress: PackProgress) => void) =>
    invoke<Skin[]>("community_add", { packId, onProgress: new Channel<PackProgress>(onProgress) }),
  /** Every skin of a pack drawn as its folder, to look through before adding it. Saves nothing. */
  /** Downloads and draws a pack to look through, or shows it as drawn before when this version
   *  (`hash`) was looked at in the last week. */
  packSkins: (packId: string, hash: string) => invoke<PackSkinPreview[]>("community_pack_skins", { packId, hash }),
  /** Swaps an added pack's skins for the version published now. `onProgress` hears how far it has got. */
  updatePack: (packId: string, onProgress?: (progress: PackProgress) => void) =>
    invoke<PackUpdate>("community_update", { packId, onProgress: new Channel<PackProgress>(onProgress) }),
  /** Deletes a pack's skins; resolves to their ids. */
  removePack: (packId: string) => invoke<string[]>("community_remove", { packId }),
  /** Adds a pack from a folder on this computer. */
  importPack: (path: string) => invoke<Skin[]>("import_pack", { path }),
  /** Writes skins as a pack folder inside `folder`, named after a new id; resolves to the folder it
   *  made and the pictures made smaller to fit. `onProgress` hears how many pictures are ready. */
  exportPack: (req: ExportPackRequest, onProgress: (p: MakeProgress) => void) =>
    invoke<ExportedPack>("export_pack", { ...req, onProgress: new Channel<MakeProgress>(onProgress) }),
  // ---- first launch ----
  /** True until the first-launch onboarding has been finished on this computer. */
  onboardingNeeded: () => invoke<boolean>("onboarding_needed"),
  /** Remembers that the onboarding is done, so it never shows again. */
  finishOnboarding: () => invoke<void>("finish_onboarding"),

  /** Native window appearance; `null` follows the system. Keeps the macOS sidebar material in step with the app theme. */
  setWindowTheme: (theme: "light" | "dark" | null) => getCurrentWindow().setTheme(theme),
  /** Puts the menu bar in `language` with `menu`'s words; with `pin`, macOS's own panels follow it from the next launch (language.rs). */
  setLanguage: (language: string, menu: Record<string, string>, pin: boolean) => invoke<void>("set_language", { language, menu, pin }),

  // ---- AI assistant (bring your own key) ----
  aiCatalogue: () => invoke<AiCatalogue>("ai_catalogue"),
  aiSetKey: (provider: string, key: string) => invoke<void>("ai_set_key", { provider, key }),
  aiClearKey: (provider: string) => invoke<void>("ai_clear_key", { provider }),
  aiTestKey: (provider: string) => invoke<void>("ai_test_key", { provider }),
  /** Makes a picture and saves it as a skin, telling `onEvent` how it's going as it goes. */
  aiGenerate: (req: AiGenerateRequest, onEvent: (event: AiEvent) => void = () => {}) =>
    invoke<Skin>("ai_generate", { req, onEvent: new Channel<AiEvent>(onEvent) }),
  /** Stops the run named `job`: its request is dropped, or its local model stopped; {@link LOCAL_SETUP_JOB} stops setting up. */
  aiCancel: (job: string) => invoke<void>("ai_cancel", { job }),
  /** Whether the local model can paint on this machine, and what setting it up takes. */
  aiLocalStatus: () => invoke<LocalStatus>("ai_local_status"),
  /** Removes the local model: what setup downloaded, and mflux when setup installed it. Refused ("busy") while it is being set up or is painting. */
  aiLocalRemove: () => invoke<LocalStatus>("ai_local_remove"),
  /** Removes the model files an earlier setup left that the model doesn't use now, and says how it stands after. */
  aiLocalRemoveUnused: () => invoke<LocalStatus>("ai_local_remove_unused"),
  /** Downloads and checks the runtime and model for this computer, telling `onEvent` as it goes.
   *  Stopped with `aiCancel(LOCAL_SETUP_JOB)` (it fails with the code "stopped"); what was downloaded is kept.
   *  Asked while a setup is under way, it joins that one: `onEvent` hears where it has got to, and it settles as that one does. */
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
  composerTemplate: (style: FolderStyle) => invoke<ComposerTemplate>("composer_template", { style }),
  /** Saves a design (its full-size picture and its document) as a skin, or changes one saved before. */
  composerSave: (header: ComposerSaveHeader, png: Uint8Array) => invoke<ComposerSaved>("composer_save", frame(header, png)),
  /** The design as the icon at each of `sizes`, as data URLs, drawn by the compositor. */
  composerPreview: (shape: "folder" | "free", style: FolderStyle, sizes: number[], png: Uint8Array) =>
    invoke<string[]>("composer_preview", frame({ shape, style, sizes }, png)),
  /** A picture file, read (and shrunk) for a picture layer. */
  composerImage: (path: string) => invoke<ComposerImage>("composer_image", { path }),
  /** A saved skin's own picture, for a picture layer or a remix. */
  composerSkinImage: (skinId: string) => invoke<ComposerImage>("composer_skin_image", { skinId }),
  /** The document of a saved design, to edit it again; null for a skin that wasn't made in the composer. */
  composerDesign: (skinId: string) => invoke<unknown>("composer_design", { skinId }),

  // ---- sharing a pack (src-tauri/src/share.rs) ----
  /** Whether it can be used here, and who this computer is to the service. A build with no
   *  service says so without asking anything over the network. */
  shareStatus: () => invoke<ShareStatus>("share_status"),
  /** The signed page that verifies this computer under `handle`, to open in the browser. */
  shareVerify: (handle: string) => invoke<string>("share_verify", { handle }),
  /** Resolves once the service says the browser check is done. */
  shareWait: () => invoke<ShareStatus>("share_wait"),
  shareCancel: () => invoke<void>("share_cancel"),
  /** Saves this computer's sharing key to a recovery file at `path`. */
  shareSaveKey: (path: string) => invoke<void>("share_save_key", { path }),
  /** Takes the key in a recovery file as this computer's. */
  shareLoadKey: (path: string) => invoke<ShareStatus>("share_load_key", { path }),
  /** Sends a pack to the review queue, trying a request again after a pause while it fails in a
   *  way that may pass; a cooldown or a ban ends it with the service's own sentence. */
  shareSubmit: (pack: PackToShare, onProgress: (p: ShareProgress) => void) =>
    invoke<SharedPack>("share_submit", { pack, onProgress: new Channel<ShareProgress>(onProgress) }),
  /** This computer's packs, newest first. */
  shareSubmissions: () => invoke<MySubmission[]>("share_submissions"),
  /** Takes one back, out of the queue or out of the community. */
  shareWithdraw: (id: string) => invoke<void>("share_withdraw", { id }),
};

/**
 * Typed wrappers over the Tauri commands exposed by `src-tauri/src/commands.rs`. `pnpm dev` in a
 * plain browser gets the stand-ins instead; a build leaves them out, since it only runs in the app.
 */
export const api: typeof tauriApi = import.meta.env.DEV && !isTauri() ? mockApi : tauriApi;

/**
 * Turns any thrown value from `invoke` into a sentence the drop zone can show, in the language on
 * show when it's one the app knows (lib/sentences.ts).
 */
export function errorMessage(err: unknown): string {
  if (typeof err === "string") return explain(err);
  if (err instanceof Error) return explain(err.message);
  // A structured error (the AI commands' {code, message}) says itself in its message.
  if (err && typeof err === "object" && typeof (err as { message?: unknown }).message === "string") return explain((err as { message: string }).message);
  return t("common.errors.somethingWrong");
}
