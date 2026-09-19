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
 */
import { COLOUR_FOLDERS } from "../assets/onboarding";
import type {
  AiCatalogue,
  AiGenerateRequest,
  CommunityPack,
  ExportPackRequest,
  PackProgress,
  PackSkinPreview,
  PackUpdate,
  PathInfo,
  PlatformInfo,
  Skin,
  SkinList,
} from "./tauri";
import { cleanName } from "./names";
import { cleanTags } from "./tags";

/** Keys "saved" in the browser preview, so the assistant can be walked through end to end. */
const mockKeys = new Set<string>();

/** Sample packs for the browser preview's Community view. The real list comes from GitHub. */
const MOCK_PACKS: Omit<CommunityPack, "added" | "update">[] = [
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

/** Everything in the preview's library, newest first. */
let library: Skin[] = [];
/** Sample packs added before their "new version": Colours gets one the first time it is added. */
const mockStale = new Set<string>();
/** Packs that have already failed once, so the next try works. */
const mockFailedOnce = new Set<string>();

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
function packPictures(pack: Omit<CommunityPack, "added" | "update">): { name: string; thumbnail: string }[] {
  return Array.from({ length: pack.count }, (_, i) => {
    if (pack.id === "classic-art") return { name: CLASSIC_ART[i][1], thumbnail: `/community/packs/classic-art/${CLASSIC_ART[i][0]}.webp` };
    if (pack.id === "colours") return { name: COLOUR_NAMES[i % 4] + (i >= 4 ? " 2" : ""), thumbnail: COLOUR_FOLDERS[i % 4] };
    return { name: `${pack.name} ${i + 1}`, thumbnail: picture(i + 5) };
  });
}

function mockPackSkins(pack: Omit<CommunityPack, "added" | "update">): Skin[] {
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

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const mockApi = {
  listSkins: async (): Promise<SkinList> => ({
    skins: [...library].sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0)),
    default_thumbnail: COLOUR_FOLDERS[0],
  }),
  inspectPath: async (path: string): Promise<PathInfo> => ({ kind: "folder", name: path.split(/[\\/]/).pop() || path, path }),
  importImage: async (path: string): Promise<Skin> => {
    await sleep(500);
    const name = (path.split(/[\\/]/).pop() || "Your picture").replace(/\.[^.]+$/, "");
    const skin: Skin = { id: `user:${Date.now()}`, name, collection: "yours", thumbnail: picture(library.length), custom: true, kind: "artwork", source: "import", created_at: Date.now(), tags: [] };
    keep([skin]);
    return skin;
  },
  applySkin: async () => new Promise<void>((r) => setTimeout(r, 600)),
  revertSkin: async () => new Promise<void>((r) => setTimeout(r, 400)),
  platformInfo: async (): Promise<PlatformInfo> => ({ os: "macos", browse_label: "your Mac", note: "browser preview: nothing is written to disk" }),
  folderIcon: async (): Promise<string> => COLOUR_FOLDERS[0],
  skinsFolder: async () => "/Users/you/Library/Application Support/app.folderskin/skins",
  deleteSkin: async (skinId: string) => {
    library = library.filter((s) => s.id !== skinId);
  },
  editSkin: async (_skinId: string, name: string, tags: string[]) => ({ name: cleanName(name), tags: cleanTags(tags) }),
  communityPacks: async (_fresh = false): Promise<CommunityPack[]> => {
    await sleep(500);
    if (offline()) throw OFFLINE;
    return listed().map((p) => ({ ...p, added: packAdded(p.id), update: packAdded(p.id) && mockStale.has(p.id) }));
  },
  communityPreview: async (packId: string) => `/community/previews/${PREVIEW_OF[packId] ?? "colours"}.png`,
  addPack: async (packId: string, onProgress?: (progress: PackProgress) => void): Promise<Skin[]> => {
    const pack = MOCK_PACKS.find((p) => p.id === packId);
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
  packSkins: async (packId: string): Promise<PackSkinPreview[]> => {
    await sleep(900);
    const pack = MOCK_PACKS.find((p) => p.id === packId);
    if (!pack) throw "that isn't a pack";
    return packPictures(pack).map((p) => ({ ...p, tags: pack.tags }));
  },
  updatePack: async (packId: string): Promise<PackUpdate> => {
    await sleep(1200);
    const pack = MOCK_PACKS.find((p) => p.id === packId);
    if (!pack) throw "that isn't a pack";
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
  aiGenerate: async (_req: AiGenerateRequest): Promise<Skin> => {
    await sleep(4200);
    const name = _req.idea.split(/\s+/).slice(0, 4).join(" ");
    const skin: Skin = { id: `user:ai${Date.now()}`, name: name.charAt(0).toUpperCase() + name.slice(1), collection: "yours", thumbnail: picture(library.length + 1), custom: true, kind: _req.shape === "folder" ? "folder" : "artwork", source: "ai", created_at: Date.now(), tags: cleanTags(_req.tags), made_with: "OpenAI · GPT Image 2.5 Flare", idea: _req.idea };
    keep([skin]);
    return skin;
  },
};
