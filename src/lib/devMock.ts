/**
 * Browser-only stand-in for the Tauri commands so `pnpm dev` in a plain browser shows the
 * real layout with the generated previews. Vite serves them from assets/previews in dev only,
 * so they never reach a build (the app renders its own thumbnails).
 * Never used inside the app: `isTauri()` is true there.
 */
import type { AiCatalogue, AiGenerateRequest, CommunityPack, ExportPackRequest, PathInfo, PlatformInfo, Skin, SkinList } from "./tauri";
import { cleanName } from "./names";
import { cleanTags } from "./tags";

const IDS: [string, string, string][] = [
  ["aurora", "Aurora", "glow"],
  ["sunset", "Sunset", "glow"],
  ["mesh", "Mesh", "glow"],
  ["ember", "Ember", "glow"],
  ["paper", "Paper", "grain"],
  ["denim", "Denim", "grain"],
  ["slate", "Slate", "grain"],
  ["halftone", "Halftone", "pop"],
  ["stripes", "Stripes", "pop"],
  ["bubbles", "Bubbles", "pop"],
];

/** Keys "saved" in the browser preview, so the assistant can be walked through end to end. */
const mockKeys = new Set<string>();

/** Sample packs for the browser preview's Community view. The real list comes from GitHub. */
const MOCK_PACKS: Omit<CommunityPack, "added">[] = [
  { id: "classic-art", name: "Classic Art", author: "prajwal-svm", license: "CC0-1.0", tags: ["classic art"], count: 16 },
  { id: "colours", name: "Colours", author: "prajwal-svm", license: "CC0-1.0", tags: ["colour"], count: 8 },
  { id: "night-prints", name: "Night prints", author: "example", license: "CC-BY-4.0", tags: ["woodblock", "night", "animals"], count: 12 },
  { id: "chrome-dreams", name: "Chrome dreams", author: "example", license: "CC-BY-4.0", tags: ["airbrush", "retro"], count: 6 },
];
/** The real preview strips from community/previews; the made-up packs borrow one. */
const PREVIEW_OF: Record<string, string> = { "classic-art": "classic-art", colours: "colours", "night-prints": "classic-art", "chrome-dreams": "colours" };
/** The skins each added sample pack put in the library. */
const mockAdded = new Map<string, string[]>();

function mockPackSkins(pack: Omit<CommunityPack, "added">): Skin[] {
  const pictures = ["aurora", "sunset", "mesh", "ember", "paper", "denim", "slate", "halftone"];
  return Array.from({ length: Math.min(pack.count, 4) }, (_, i) => ({
    id: `user:${pack.id}${i}`,
    name: `${pack.name} ${i + 1}`,
    collection: "community",
    thumbnail: `/assets/previews/${pictures[i % pictures.length]}.png`,
    custom: true,
    kind: "artwork" as const,
    source: "community" as const,
    created_at: Date.now() + i,
    tags: pack.tags,
    pack: pack.id,
    pack_name: pack.name,
    author: pack.author,
    license: pack.license,
  }));
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const mockApi = {
  listSkins: async (): Promise<SkinList> => ({
    skins: [
      ...IDS.map(([id, name, collection]) => ({
        id,
        name,
        collection,
        thumbnail: `/assets/previews/${id}.png`,
        custom: false,
        kind: "artwork" as const,
        source: "builtin" as const,
        created_at: null,
        tags: [collection],
      })),
    ],
    default_thumbnail: "/assets/previews/mesh.png",
  }),
  inspectPath: async (path: string): Promise<PathInfo> => ({ kind: "folder", name: path.split(/[\\/]/).pop() || path, path }),
  importImage: async (path: string): Promise<Skin> => {
    await new Promise((r) => setTimeout(r, 500));
    const name = (path.split(/[\\/]/).pop() || "Your picture").replace(/\.[^.]+$/, "");
    return { id: `user:${Date.now()}`, name, collection: "yours", thumbnail: "/assets/previews/ember.png", custom: true, kind: "artwork", source: "import", created_at: Date.now(), tags: [] };
  },
  applySkin: async () => new Promise<void>((r) => setTimeout(r, 600)),
  revertSkin: async () => new Promise<void>((r) => setTimeout(r, 400)),
  platformInfo: async (): Promise<PlatformInfo> => ({ os: "macos", browse_label: "your Mac", note: "browser preview: nothing is written to disk" }),
  folderIcon: async (): Promise<string> => "/assets/previews/mesh.png",
  skinsFolder: async () => "/Users/you/Library/Application Support/app.folderskin/skins",
  deleteSkin: async () => {},
  editSkin: async (_skinId: string, name: string, tags: string[]) => ({ name: cleanName(name), tags: cleanTags(tags) }),
  communityPacks: async (): Promise<CommunityPack[]> => {
    await new Promise((r) => setTimeout(r, 500));
    return MOCK_PACKS.map((p) => ({ ...p, added: mockAdded.has(p.id) }));
  },
  communityPreview: async (packId: string) => `/community/previews/${PREVIEW_OF[packId] ?? "colours"}.png`,
  addPack: async (packId: string): Promise<Skin[]> => {
    await new Promise((r) => setTimeout(r, 1200));
    const pack = MOCK_PACKS.find((p) => p.id === packId);
    if (!pack) throw "that isn't a pack";
    const skins = mockPackSkins(pack);
    mockAdded.set(packId, skins.map((s) => s.id));
    return skins;
  },
  removePack: async (packId: string): Promise<string[]> => {
    const ids = mockAdded.get(packId) ?? [];
    mockAdded.delete(packId);
    return ids;
  },
  importPack: async (path: string): Promise<Skin[]> => {
    const id = (path.split(/[\\/]/).pop() || "my-pack").toLowerCase();
    const skins = mockPackSkins({ id, name: "Folder pack", author: "you", license: "CC0-1.0", tags: ["test"], count: 2 });
    mockAdded.set(id, skins.map((s) => s.id));
    return skins;
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
    await new Promise((r) => setTimeout(r, 4200));
    const name = _req.idea.split(/\s+/).slice(0, 4).join(" ");
    return { id: `user:ai${Date.now()}`, name: name.charAt(0).toUpperCase() + name.slice(1), collection: "yours", thumbnail: "/assets/previews/bubbles.png", custom: true, kind: _req.shape === "folder" ? "folder" : "artwork", source: "ai", created_at: Date.now(), tags: cleanTags(_req.tags), made_with: "OpenAI · GPT Image 2.5 Flare", idea: _req.idea };
  },
};
