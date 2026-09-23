/**
 * Browser-only stand-in for the Tauri commands so `pnpm dev` in a plain browser shows the real
 * layout. The library lives in memory and starts empty, like a first launch; community packs use
 * the pictures in community/packs (Vite serves the repository in dev only, so they never reach a
 * build) and the intro's colour folders.
 * Never used inside the app: `isTauri()` is true there.
 *
 * The onboarding shows until it's finished once in this browser; add `?onboarding` to the address
 * to see it again. "Chrome dreams" fails the first time it's added, to show what a failure does,
 * and `?offline` makes everything from GitHub fail, as it does without a connection. `?real` leaves
 * out the made-up packs, for screenshots.
 *
 * `?update` finds a made-up next version a few seconds after the app opens, as a release build
 * does; `?update=fail` stops its download halfway and `?update=offline` can't check at all.
 *
 * `?yours=8` starts with eight skins of your own, for the parts that need a library to work on.
 *
 * `?packs=10000` adds that many made-up packs to Community (mockCommunity.ts), searched the way
 * the app searches its catalog, to see and test the view at the size it is built for.
 */
import { COLOUR_FOLDERS } from "../assets/onboarding";
import { drawOnFolder, loadTemplate, type TemplateImages } from "../composer/composite";
import { FALLBACK_PARTS } from "../composer/parts";
import type {
  AiCatalogue,
  AiGenerateRequest,
  CommunityPack,
  CommunityQuery,
  CommunitySearch,
  ComposerImage,
  ComposerSaved,
  ComposerSaveHeader,
  ComposerTemplate,
  ExportPackRequest,
  FolderIcon,
  GithubAccount,
  PackProgress,
  PackToPublish,
  PublishProgress,
  PackSkinPreview,
  PackUpdate,
  PathInfo,
  PlatformInfo,
  Skin,
  SkinList,
} from "./tauri";
import type { AvailableUpdate } from "./updater";
import type { Subfolders, TreeProgress, TreeRunResult } from "./tree";
import { cleanName } from "./names";
import { isImagePath } from "./files";
import { cleanTags } from "./tags";
import { madeUpPacks, MockCatalog, type MockPack as CatalogPack } from "./mockCommunity";

/** Keys "saved" in the browser preview, so the assistant can be walked through end to end. */
const mockKeys = new Set<string>();

/** A pack as the preview lists it, before whether it's added (or changed) is worked out. Made-up
 *  packs also carry their skins' names. */
type MockPack = Omit<CommunityPack, "added" | "update" | "hash" | "bytes" | "preview"> & { skins?: string[] };

/** Sample packs for the browser preview's Community view. The real list comes from GitHub. */
const MOCK_PACKS: MockPack[] = [
  { id: "classic-art", name: "Classic Art", author: "prajwal-svm", license: "CC0-1.0", tags: ["classic art"], count: 16 },
  { id: "colours", name: "Colours", author: "prajwal-svm", license: "CC0-1.0", tags: ["colour"], count: 8 },
  { id: "night-prints", name: "Night prints", author: "example", license: "CC-BY-4.0", tags: ["woodblock", "night", "animals"], count: 12 },
  { id: "chrome-dreams", name: "Chrome dreams", author: "example", license: "CC-BY-4.0", tags: ["airbrush", "retro"], count: 6 },
];
/** The real preview strips from community/previews; the made-up packs borrow one. */
const PREVIEW_OF: Record<string, string> = { "classic-art": "classic-art", colours: "colours", "night-prints": "classic-art", "chrome-dreams": "colours" };
/** Classic Art's pictures, finished folders already, in the pack's order. */
const CLASSIC_ART = [
  ["mona-lisa", "Mona Lisa"],
  ["view-of-toledo", "View of Toledo"],
  ["girl-with-a-pearl-earring", "Girl with a Pearl Earring"],
  ["the-astronomer", "The Astronomer"],
  ["oath-of-the-horatii", "Oath of the Horatii"],
  ["napoleon-crossing-the-alps", "Napoleon Crossing the Alps"],
  ["wanderer-above-the-sea-of-fog", "Wanderer above the Sea of Fog"],
  ["the-ninth-wave", "The Ninth Wave"],
  ["boulevard-des-capucines", "Boulevard des Capucines"],
  ["breezing-up", "Breezing Up"],
  ["paris-street-rainy-day", "Paris Street; Rainy Day"],
  ["luncheon-of-the-boating-party", "Luncheon of the Boating Party"],
  ["the-lady-of-shalott", "The Lady of Shalott"],
  ["the-starry-night", "The Starry Night"],
  ["mont-sainte-victoire", "Mont Sainte-Victoire"],
  ["composition-viii", "Composition VIII"],
] as const;
const COLOUR_NAMES = ["Blue", "Orange", "Purple", "Green"];

/** `?yours=8` starts with that many skins of your own, so sharing can be tried without making any. */
function seeded(): Skin[] {
  const many = Number(new URLSearchParams(location.search).get("yours") ?? "0");
  if (!Number.isFinite(many) || many < 1) return [];
  const now = Date.now();
  return Array.from({ length: Math.min(many, 40) }, (_, i) => ({
    id: `user:seed${i}`,
    name: CLASSIC_ART[i % CLASSIC_ART.length][1],
    collection: "yours",
    thumbnail: picture(i),
    custom: true,
    kind: "artwork" as const,
    source: "import" as const,
    created_at: now - i,
    tags: i % 3 === 0 ? ["painting"] : ["photo"],
  }));
}

/** Everything in the preview's library, newest first. */
let library: Skin[] = seeded();
/** Sample packs added before their "new version": Colours gets one the first time it is added. */
const mockStale = new Set<string>();
/** Packs that have already failed once, so the next try works. */
const mockFailedOnce = new Set<string>();
/** Packs looked through already, which the app would show from its cache. */
const mockViewed = new Set<string>();

const ONBOARDED_KEY = "folderskin.mock.onboarded";
const OFFLINE = "couldn't reach GitHub. Check your connection and try again";
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const offline = () => new URLSearchParams(location.search).has("offline");
/** The packs that really are on GitHub; the others only show what a longer list looks like. */
const REAL_PACKS = new Set(["classic-art", "colours"]);
const listed = () => (new URLSearchParams(location.search).has("real") ? MOCK_PACKS.filter((p) => REAL_PACKS.has(p.id)) : MOCK_PACKS);

/** A stand-in skin picture: one of Classic Art's, or one of the intro's colour folders. */
function picture(i: number): string {
  return i % 3 === 2 ? COLOUR_FOLDERS[i % COLOUR_FOLDERS.length] : `/community/packs/classic-art/${CLASSIC_ART[i % CLASSIC_ART.length][0]}.webp`;
}

/** What a pack's skins look like here: the real pictures for Classic Art and Colours. */
function packPictures(pack: MockPack): { name: string; thumbnail: string }[] {
  return Array.from({ length: pack.count }, (_, i) => {
    if (pack.id === "classic-art") return { name: CLASSIC_ART[i][1], thumbnail: `/community/packs/classic-art/${CLASSIC_ART[i][0]}.webp` };
    if (pack.id === "colours") return { name: COLOUR_NAMES[i % 4] + (i >= 4 ? " 2" : ""), thumbnail: COLOUR_FOLDERS[i % 4] };
    return { name: pack.skins?.[i] ?? `${pack.name} ${i + 1}`, thumbnail: picture(i + 5) };
  });
}

/** The real preview strips, which the made-up packs borrow in turn. */
const REAL_PREVIEWS = ["classic-art", "colours", "greek-art", "scientists-pop-art", "soft-rainbow"].map((id) => `/community/previews/${id}.png`);
/** When the sample packs were published, newest first. */
const SAMPLE_DATES: Record<string, number> = { "classic-art": 1_780_000_000, colours: 1_770_000_000, "night-prints": 1_760_000_000, "chrome-dreams": 1_750_000_000 };

let mockCatalogue: MockCatalog | null = null;
/** Whether the preview's catalog has been "downloaded" yet: the first search waits for it, as in the app. */
let mockCatalogueLoaded = false;

/** The preview's catalog: the sample packs, then `?packs=N` made-up ones, made the first time it is asked for. */
function communityCatalog(): MockCatalog {
  if (mockCatalogue) return mockCatalogue;
  const many = Math.min(Math.max(Number(new URLSearchParams(location.search).get("packs") ?? "0") || 0, 0), 50_000);
  const samples: CatalogPack[] = listed().map((p) => ({
    ...p,
    hash: "",
    bytes: p.count * 180_000,
    added: SAMPLE_DATES[p.id] ?? 0,
    skins: packPictures(p).map((s) => s.name),
    preview: `/community/previews/${PREVIEW_OF[p.id] ?? "colours"}.png`,
  }));
  mockCatalogue = new MockCatalog([...samples, ...madeUpPacks(many, REAL_PREVIEWS)], ["classic-art", "colours"]);
  return mockCatalogue;
}

/** A catalog pack as the webview gets it, marked against the library. */
function communityPack(p: CatalogPack): CommunityPack {
  const added = packAdded(p.id);
  return {
    id: p.id,
    name: p.name,
    author: p.author,
    license: p.license,
    tags: p.tags,
    count: p.count,
    bytes: p.bytes,
    hash: p.hash,
    preview: p.preview,
    added,
    update: added && mockStale.has(p.id),
  };
}

/** Any pack the preview knows, sample or made up. */
const findPack = (id: string): MockPack | undefined => MOCK_PACKS.find((p) => p.id === id) ?? communityCatalog().find(id);

function mockPackSkins(pack: MockPack): Skin[] {
  const now = Date.now();
  return packPictures(pack).map((p, i) => ({
    id: `user:${pack.id}${i}`,
    name: p.name,
    collection: "yours",
    thumbnail: p.thumbnail,
    custom: true,
    kind: "folder" as const,
    source: "community" as const,
    // The pack's first skin is the newest, as the app does it, so the library shows the pack in order.
    created_at: now + pack.count - i,
    tags: pack.tags,
    pack: pack.id,
    pack_name: pack.name,
    author: pack.author,
    license: pack.license,
  }));
}

/** Puts skins in the library, replacing any with the same id. */
function keep(skins: Skin[]) {
  library = [...skins, ...library.filter((s) => !skins.some((k) => k.id === s.id))];
}

const packAdded = (id: string) => library.some((s) => s.pack === id);

/** Folders the preview's "choose a folder" hands out in turn, so switching folders can be tried. */
const SAMPLE_FOLDERS = ["/Users/you/Documents/Projects", "/Users/you/Pictures/Wedding", "/Users/you/Desktop/Taxes 2026", "/Users/you/Pictures/Photo archive"];
/** Too big for a run: its switch can't be turned on. */
const HUGE_TREE = "/Users/you/Pictures/Photo archive";
let nextSample = 0;
/**
 * The icon each folder wears in the preview: Projects starts plain and the others with a colour
 * of their own, so a custom icon can be tried. Applying and reverting change it.
 */
const mockIcons = new Map<string, string | null>(SAMPLE_FOLDERS.map((path, i) => [path, i === 0 ? null : COLOUR_FOLDERS[i]]));

/**
 * The folders inside each sample folder, for trying "Include subfolders": Projects has plenty,
 * Wedding has one the preview can't change (to show a partial result), Taxes 2026 has none, and
 * Photo archive has more than a run takes.
 */
const SAMPLE_TREES: Record<string, string[]> = {
  "/Users/you/Documents/Projects": [
    "Clients", "Design", "Invoices", "Notes", "Photos", "Research", "Templates", "Videos",
    "Clients/Acme", "Clients/Globex", "Clients/Initech", "Design/Icons", "Design/Mockups",
    "Invoices/2025", "Invoices/2026", "Photos/2019", "Photos/2020", "Photos/2021", "Research/Papers",
    "Templates/Letters", "Videos/Raw", "Videos/Edited", "Clients/Acme/Contracts", "Photos/2021/Holiday",
  ].map((p) => `/Users/you/Documents/Projects/${p}`),
  "/Users/you/Pictures/Wedding": ["Ceremony", "Guests", "Private", "Reception", "Ceremony/Rings", "Reception/Speeches"].map(
    (p) => `/Users/you/Pictures/Wedding/${p}`,
  ),
};
/** Folders in the sample trees that wear an icon of their own. */
const mockTreeIcons = new Set<string>(["/Users/you/Pictures/Wedding/Guests"]);
let mockStop = false;
const lastPart = (path: string) => path.split("/").pop() || path;

async function mockTreeRun(
  root: string,
  only: string[] | null,
  onProgress: (p: TreeProgress) => void,
  act: (path: string) => "changed" | "skipped" | string,
): Promise<TreeRunResult> {
  mockStop = false;
  const plan = only ?? [root, ...(SAMPLE_TREES[root] ?? [])];
  const result: TreeRunResult = { total: plan.length, changed: [], failed: [], skipped: 0, remaining: [], stopped: false };
  onProgress({ done: 0, total: plan.length, name: lastPart(root) });
  for (let i = 0; i < plan.length; i++) {
    if (mockStop) {
      result.stopped = true;
      result.remaining = plan.slice(i);
      break;
    }
    await sleep(140);
    const path = plan[i];
    const outcome = act(path);
    if (outcome === "changed") result.changed.push(path);
    else if (outcome === "skipped") result.skipped += 1;
    else result.failed.push({ path, name: lastPart(path), reason: outcome });
    onProgress({ done: i + 1, total: plan.length, name: lastPart(path) });
  }
  return result;
}

/** The next sample folder, for the preview's "choose a folder". */
export function mockPickFolder(): string {
  const path = SAMPLE_FOLDERS[nextSample % SAMPLE_FOLDERS.length];
  nextSample += 1;
  return path;
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Notes for the made-up update, written the way a CHANGELOG.md section is. */
const MOCK_NOTES = `### Added

- FolderSkin updates itself: it looks for a new version when it opens, shows what changed, and
  restarts into it.
- A **Scientists - Pop Art** community pack.

### Fixed

- Cut-outs on a plain grey or black background keep dark clothes and ink lines.

---

**Downloads:** macOS \`.dmg\`, Windows \`-setup.exe\` or \`.msi\`, Linux \`.AppImage\`, \`.deb\` or \`.rpm\`.`;

/** The browser preview's update check: see `?update` at the top. */
export async function mockFindUpdate(): Promise<AvailableUpdate | null> {
  const mode = new URLSearchParams(location.search).get("update");
  await sleep(900);
  if (mode === null) return null;
  if (mode === "offline" || offline()) throw new Error("Could not fetch a valid release JSON from the remote");
  return {
    version: "0.2.0",
    notes: MOCK_NOTES,
    install: async (onProgress) => {
      onProgress(0);
      for (let step = 1; step <= 24; step++) {
        await sleep(110);
        if (mode === "fail" && step === 13) throw new Error("error decoding response body");
        onProgress(step / 24);
      }
      await sleep(500);
    },
  };
}

/**
 * The composer's folder layers in the browser preview: the same pictures the Rust side makes, as
 * `folderskin-tools composer-layers` wrote them into docs/images/composer (served in dev only).
 */
const MOCK_TEMPLATE_URLS = {
  back: "/docs/images/composer/back.png",
  front: "/docs/images/composer/front.png",
  middle: "/docs/images/composer/middle.png",
  top: "/docs/images/composer/top.png",
  outline: "/docs/images/composer/outline.png",
};
let mockTemplate: Promise<TemplateImages> | null = null;
const templateImages = () => (mockTemplate ??= loadTemplate(MOCK_TEMPLATE_URLS));

/** Designs "saved" in the browser preview, by skin id, so Edit design can be tried. */
const mockDesigns = new Map<string, unknown>();

function base64(bytes: Uint8Array): string {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

function loadImg(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("couldn't read that picture"));
    img.src = src;
  });
}

/** What the Rust side would render for a design: on the folder, or as it is for a free icon. */
async function mockIcon(png: Uint8Array, shape: "folder" | "free", size: number): Promise<string> {
  const img = await loadImg(`data:image/png;base64,${base64(png)}`);
  const c = document.createElement("canvas");
  c.width = size;
  c.height = size;
  const ctx = c.getContext("2d")!;
  ctx.imageSmoothingQuality = "high";
  if (shape === "folder") drawOnFolder(ctx, img, await templateImages(), size, document.createElement("canvas"));
  else ctx.drawImage(img, 0, 0, size, size);
  return c.toDataURL("image/png");
}

/** A made-up photo for "Choose a picture" in the browser: a lake at sunset, no file needed. */
function mockPhoto(): ComposerImage {
  const c = document.createElement("canvas");
  c.width = 1600;
  c.height = 1000;
  const g = c.getContext("2d")!;
  const sky = g.createLinearGradient(0, 0, 0, 640);
  sky.addColorStop(0, "#2b1f5c");
  sky.addColorStop(0.55, "#e0628a");
  sky.addColorStop(1, "#ffbf6b");
  g.fillStyle = sky;
  g.fillRect(0, 0, 1600, 640);
  g.fillStyle = "#fff1c9";
  g.beginPath();
  g.arc(1040, 560, 120, 0, Math.PI * 2);
  g.fill();
  g.fillStyle = "#3b2a57";
  g.beginPath();
  g.moveTo(0, 640);
  g.bezierCurveTo(300, 420, 560, 560, 820, 500);
  g.bezierCurveTo(1100, 440, 1320, 560, 1600, 470);
  g.lineTo(1600, 1000);
  g.lineTo(0, 1000);
  g.fill();
  const lake = g.createLinearGradient(0, 640, 0, 1000);
  lake.addColorStop(0, "#f39a7d");
  lake.addColorStop(1, "#40285f");
  g.fillStyle = lake;
  g.fillRect(0, 700, 1600, 300);
  return { url: c.toDataURL("image/jpeg", 0.9), width: 1600, height: 1000, name: "Lake at sunset", alpha: false };
}

/** Who the mock is pretending is signed in. */
const mockGithub: { account: GithubAccount | null } = { account: null };

export const mockApi = {
  listSkins: async (): Promise<SkinList> => ({
    skins: [...library].sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0)),
    default_thumbnail: COLOUR_FOLDERS[0],
  }),
  githubAccount: async () => mockGithub.account,
  githubConnect: async () => {
    mockGithub.account = null;
    return { user_code: "WDJB-MJHT", verification_uri: "https://github.com/login/device", expires_in: 900 };
  },
  githubWait: async () => {
    await new Promise((r) => setTimeout(r, 2500));
    mockGithub.account = { login: "octocat", name: "The Octocat", avatar_url: "" };
    return mockGithub.account;
  },
  githubCancel: async () => {},
  githubSignOut: async () => {
    mockGithub.account = null;
  },
  publishPack: async (pack: PackToPublish, onProgress: (p: PublishProgress) => void) => {
    const steps: PublishProgress[] = [{ stage: "checking" }, { stage: "forking" }, { stage: "branching" }];
    for (const step of steps) {
      onProgress(step);
      await new Promise((r) => setTimeout(r, 600));
    }
    const total = pack.skinIds.length + 1;
    for (let done = 0; done <= total; done++) {
      onProgress({ stage: "uploading", done, total });
      await new Promise((r) => setTimeout(r, 120));
    }
    onProgress({ stage: "opening" });
    await new Promise((r) => setTimeout(r, 600));
    return { url: "https://github.com/prajwal-svm/folderskin/pull/42", number: 42, forked: true };
  },
  inspectPath: async (path: string): Promise<PathInfo> => ({
    kind: isImagePath(path) ? "image" : "folder",
    name: path.split(/[\\/]/).pop() || path,
    path,
  }),
  importImage: async (path: string): Promise<Skin> => {
    await sleep(500);
    const name = (path.split(/[\\/]/).pop() || "Your picture").replace(/\.[^.]+$/, "");
    const skin: Skin = { id: `user:${Date.now()}`, name, collection: "yours", thumbnail: picture(library.length), custom: true, kind: "artwork", source: "import", created_at: Date.now(), tags: [] };
    keep([skin]);
    return skin;
  },
  applySkin: async (folder: string, skinId: string) => {
    await sleep(600);
    mockIcons.set(folder, library.find((s) => s.id === skinId)?.thumbnail ?? null);
  },
  revertSkin: async (folder: string) => {
    await sleep(400);
    mockIcons.set(folder, null);
  },
  platformInfo: async (): Promise<PlatformInfo> => ({ os: "macos", browse_label: "your Mac", note: "browser preview: nothing is written to disk" }),
  subfolderCount: async (folder: string): Promise<Subfolders> => {
    await sleep(260);
    if (folder === HUGE_TREE) return { count: 5000, more: true };
    return { count: SAMPLE_TREES[folder]?.length ?? 0, more: false };
  },
  treeBytes: async (_skinId: string): Promise<number> => {
    await sleep(200);
    // What the app measured for a painted skin on macOS.
    return 2_670_631;
  },
  applySkinTree: async (folder: string, skinId: string, only: string[] | null, onProgress: (p: TreeProgress) => void): Promise<TreeRunResult> => {
    const thumb = library.find((s) => s.id === skinId)?.thumbnail ?? null;
    const result = await mockTreeRun(folder, only, onProgress, (path) => {
      if (path.endsWith("/Private")) return "you don't have permission to change it";
      mockTreeIcons.add(path);
      return "changed";
    });
    if (result.changed.includes(folder)) mockIcons.set(folder, thumb);
    return result;
  },
  revertSkinTree: async (folder: string, only: string[] | null, onProgress: (p: TreeProgress) => void): Promise<TreeRunResult> => {
    const result = await mockTreeRun(folder, only, onProgress, (path) => {
      const has = mockTreeIcons.has(path) || (path === folder && mockIcons.get(folder) != null);
      if (!only && !has) return "skipped";
      mockTreeIcons.delete(path);
      return "changed";
    });
    if (result.changed.includes(folder)) mockIcons.set(folder, null);
    return result;
  },
  stopTreeRun: async () => {
    mockStop = true;
  },
  folderIcon: async (path: string): Promise<FolderIcon> => {
    await sleep(120);
    const custom = mockIcons.get(path) ?? null;
    return { url: custom ?? COLOUR_FOLDERS[0], custom: custom !== null };
  },
  skinsFolder: async () => "/Users/you/Library/Application Support/app.folderskin/skins",
  deleteSkin: async (skinId: string) => {
    library = library.filter((s) => s.id !== skinId);
  },
  editSkin: async (_skinId: string, name: string, tags: string[]) => ({ name: cleanName(name), tags: cleanTags(tags) }),
  communityPacks: async (_fresh = false): Promise<CommunityPack[]> => {
    await sleep(500);
    if (offline()) throw OFFLINE;
    const catalog = communityCatalog();
    return listed().map((p) => communityPack(catalog.find(p.id)!));
  },
  communitySearch: async (query: CommunityQuery): Promise<CommunitySearch> => {
    // Counted where the end-to-end tests can read it, to see that coming back to Community
    // didn't search again.
    const counted = window as { mockCommunitySearches?: number };
    counted.mockCommunitySearches = (counted.mockCommunitySearches ?? 0) + 1;
    // The first search waits for the catalog, as the app's does; after that about an IPC round trip.
    await sleep(mockCatalogueLoaded ? 8 : 450);
    if (offline()) throw OFFLINE;
    mockCatalogueLoaded = true;
    const found = communityCatalog().search(query);
    const hitPacks = [...new Map(found.skins.map((h) => [h.pack.id, h.pack])).values()];
    return {
      total: found.total,
      all: found.all,
      packs: found.packs.map(communityPack),
      skins: found.skins.map((h) => ({ pack: h.pack.id, pack_name: h.pack.name, name: h.name, index: h.index, thumbnail: packPictures(h.pack)[h.index].thumbnail })),
      hit_packs: hitPacks.map(communityPack),
      facets: found.facets,
      offline: false,
    };
  },
  communityRefresh: async (): Promise<{ updates: number; packs: number }> => {
    await sleep(600);
    if (offline()) throw OFFLINE;
    const packs = communityCatalog().packs;
    return { updates: packs.filter((p) => packAdded(p.id) && mockStale.has(p.id)).length, packs: packs.length };
  },
  addPack: async (packId: string, onProgress?: (progress: PackProgress) => void): Promise<Skin[]> => {
    const pack = findPack(packId);
    if (!pack) throw "that isn't a pack";
    const total = pack.count;
    if (offline()) throw OFFLINE;
    onProgress?.({ stage: "download", done: 0, total });
    for (let done = 1; done <= total; done++) {
      await sleep(110);
      if (packId === "chrome-dreams" && !mockFailedOnce.has(packId) && done > total / 2) {
        mockFailedOnce.add(packId);
        throw OFFLINE;
      }
      onProgress?.({ stage: "download", done, total });
    }
    for (let done = 0; done <= total; done += 4) {
      onProgress?.({ stage: "save", done: Math.min(done, total), total });
      await sleep(80);
    }
    onProgress?.({ stage: "save", done: total, total });
    const skins = mockPackSkins(pack);
    keep(skins);
    if (packId === "colours" && !mockStale.has("colours-updated")) mockStale.add(packId);
    return skins;
  },
  packSkins: async (packId: string, _hash: string): Promise<PackSkinPreview[]> => {
    // Like the app's cache: slow the first time, straight away after.
    if (!mockViewed.has(packId)) await sleep(900);
    mockViewed.add(packId);
    const pack = findPack(packId);
    if (!pack) throw "that isn't a pack";
    return packPictures(pack).map((p) => ({ ...p, tags: pack.tags }));
  },
  updatePack: async (packId: string, onProgress?: (progress: PackProgress) => void): Promise<PackUpdate> => {
    const pack = findPack(packId);
    if (!pack) throw "that isn't a pack";
    for (let done = 0; done <= pack.count; done++) {
      onProgress?.({ stage: "download", done, total: pack.count });
      await sleep(1200 / Math.max(pack.count, 1));
    }
    onProgress?.({ stage: "save", done: pack.count, total: pack.count });
    mockStale.delete(packId);
    mockStale.add(`${packId}-updated`);
    const skins = mockPackSkins(pack);
    keep(skins);
    return { removed: [], skins };
  },
  removePack: async (packId: string): Promise<string[]> => {
    const ids = library.filter((s) => s.pack === packId).map((s) => s.id);
    library = library.filter((s) => s.pack !== packId);
    return ids;
  },
  importPack: async (path: string): Promise<Skin[]> => {
    const id = (path.split(/[\\/]/).pop() || "my-pack").toLowerCase();
    const skins = mockPackSkins({ id, name: "Folder pack", author: "you", license: "CC0-1.0", tags: ["test"], count: 2 });
    keep(skins);
    return skins;
  },
  onboardingNeeded: async (): Promise<boolean> => {
    if (new URLSearchParams(location.search).has("onboarding")) return true;
    try {
      return localStorage.getItem(ONBOARDED_KEY) !== "1";
    } catch {
      return true;
    }
  },
  finishOnboarding: async () => {
    try {
      localStorage.setItem(ONBOARDED_KEY, "1");
    } catch {
      // The preview shows the onboarding again next time; nothing else depends on it.
    }
  },
  exportPack: async (req: ExportPackRequest): Promise<string> => {
    await new Promise((r) => setTimeout(r, 600));
    return `${req.folder}/${req.name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")}`;
  },
  setWindowTheme: async () => {},
  aiCatalogue: async (): Promise<AiCatalogue> => ({
    providers: [
      {
        id: "openai",
        label: "OpenAI",
        models: [
          { id: "gpt-image-2.5-flare", label: "GPT Image 2.5 Flare", native_alpha: true, accepts_reference: true, sizes: ["1024x1024"], price_hint: "~$0.04 / image" },
          { id: "gpt-image-2.5-sunburst", label: "GPT Image 2.5 Sunburst", native_alpha: true, accepts_reference: true, sizes: ["1024x1024"], price_hint: "~$0.19 / image" },
        ],
        keys_url: "https://platform.openai.com/api-keys",
        docs_url: "https://platform.openai.com/docs/guides/image-generation",
        key_hint: "starts with sk-",
        has_key: mockKeys.has("openai"),
      },
      {
        id: "xai",
        label: "xAI Grok",
        models: [{ id: "grok-imagine-image", label: "Grok Imagine", native_alpha: false, accepts_reference: true, sizes: ["1024x1024"], price_hint: "~$0.02 / image" }],
        keys_url: "https://console.x.ai",
        docs_url: "https://docs.x.ai",
        key_hint: "starts with xai-",
        has_key: mockKeys.has("xai"),
      },
      ...(
        [
          ["recraft", "Recraft", "recraftv3", "Recraft V3", true, "https://www.recraft.ai/profile/api", "https://www.recraft.ai/docs", "from your Recraft profile"],
          ["google", "Google Gemini", "gemini-2.5-flash-image", "Gemini 2.5 Flash Image", false, "https://aistudio.google.com/apikey", "https://ai.google.dev/gemini-api/docs/image-generation", "from Google AI Studio"],
          ["bfl", "Black Forest Labs", "flux-pro-1.1", "FLUX 1.1 Pro", false, "https://dashboard.bfl.ai", "https://docs.bfl.ai", "from the BFL dashboard"],
          ["stability", "Stability AI", "core", "Stable Image Core", false, "https://platform.stability.ai/account/keys", "https://platform.stability.ai/docs/api-reference", "starts with sk-"],
          ["ideogram", "Ideogram", "V_3", "Ideogram v3", false, "https://ideogram.ai/manage-api", "https://developer.ideogram.ai", "from your Ideogram account"],
        ] as const
      ).map(([id, label, model, modelLabel, alpha, keys_url, docs_url, key_hint]) => ({
        id,
        label,
        models: [{ id: model, label: modelLabel, native_alpha: alpha, accepts_reference: false, sizes: ["1024x1024"], price_hint: "~$0.04 / image" }],
        keys_url,
        docs_url,
        key_hint,
        has_key: mockKeys.has(id),
      })),
    ],
    presets: [
      { id: "aurora", label: "Aurora", idea: "a night sky with green and violet aurora ribbons over dark mountains" },
      { id: "dunes", label: "Dunes", idea: "warm desert dunes at golden hour, long soft shadows" },
    ],
  }),
  aiSetKey: async (provider: string) => {
    mockKeys.add(provider);
  },
  aiClearKey: async (provider: string) => {
    mockKeys.delete(provider);
  },
  aiTestKey: async () => {},
  composerTemplate: async (): Promise<ComposerTemplate> => ({ size: 1024, ...MOCK_TEMPLATE_URLS, parts: FALLBACK_PARTS }),
  composerSave: async (header: ComposerSaveHeader, png: Uint8Array): Promise<ComposerSaved> => {
    await sleep(500);
    const old = header.replaces ? library.find((s) => s.id === header.replaces) : undefined;
    const skin: Skin = {
      id: `user:c${Date.now().toString(16)}`,
      name: cleanName(header.name) || "My design",
      collection: "yours",
      thumbnail: await mockIcon(png, header.shape, 512),
      custom: true,
      kind: "folder",
      source: "composer",
      created_at: old?.created_at ?? Date.now(),
      tags: cleanTags(header.tags),
    };
    mockDesigns.set(skin.id, header.design);
    if (old) {
      library = library.map((s) => (s.id === old.id ? skin : s));
      mockDesigns.delete(old.id);
    } else keep([skin]);
    return { skin, replaced: old ? old.id : null };
  },
  composerPreview: async (shape: "folder" | "free", sizes: number[], png: Uint8Array): Promise<string[]> =>
    Promise.all(sizes.map((size) => mockIcon(png, shape, size))),
  composerImage: async (_path: string): Promise<ComposerImage> => {
    await sleep(250);
    return mockPhoto();
  },
  composerSkinImage: async (skinId: string): Promise<ComposerImage> => {
    const skin = library.find((s) => s.id === skinId);
    if (!skin) throw "that skin isn't available any more";
    const img = await loadImg(skin.thumbnail);
    const c = document.createElement("canvas");
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    c.getContext("2d")!.drawImage(img, 0, 0);
    return { url: c.toDataURL("image/png"), width: c.width, height: c.height, name: skin.name, alpha: true };
  },
  composerDesign: async (skinId: string): Promise<unknown> => mockDesigns.get(skinId) ?? null,
  aiGenerate: async (_req: AiGenerateRequest): Promise<Skin> => {
    await sleep(4200);
    const name = _req.idea.split(/\s+/).slice(0, 4).join(" ");
    const skin: Skin = { id: `user:ai${Date.now()}`, name: name.charAt(0).toUpperCase() + name.slice(1), collection: "yours", thumbnail: picture(library.length + 1), custom: true, kind: _req.shape === "folder" ? "folder" : "artwork", source: "ai", created_at: Date.now(), tags: cleanTags(_req.tags), made_with: "OpenAI · GPT Image 2.5 Flare", idea: _req.idea };
    keep([skin]);
    return skin;
  },
};
