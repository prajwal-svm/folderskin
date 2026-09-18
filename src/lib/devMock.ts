/**
 * Browser-only stand-in for the Tauri commands so `pnpm dev` in a plain browser shows the
 * real layout with the generated previews. Vite serves them from assets/previews in dev only,
 * so they never reach a build (the app renders its own thumbnails).
 * Never used inside the app: `isTauri()` is true there.
 */
import type { AiCatalogue, AiGenerateRequest, PathInfo, PlatformInfo, Skin, SkinList } from "./tauri";

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
      })),
      { id: "user:demo1", name: "Holiday photo", collection: "yours", thumbnail: "/assets/previews/sunset.png", custom: true, kind: "artwork", source: "import", created_at: Date.now() - 86_400_000 },
      { id: "user:demo2", name: "Koi in neon rain", collection: "yours", thumbnail: "/assets/previews/aurora.png", custom: true, kind: "folder", source: "ai", created_at: Date.now() - 3_600_000 },
    ],
    default_thumbnail: "/assets/previews/mesh.png",
  }),
  inspectPath: async (path: string): Promise<PathInfo> => ({ kind: "folder", name: path.split(/[\\/]/).pop() || path, path }),
  importImage: async (path: string): Promise<Skin> => {
    await new Promise((r) => setTimeout(r, 500));
    const name = (path.split(/[\\/]/).pop() || "Your picture").replace(/\.[^.]+$/, "");
    return { id: `user:${Date.now()}`, name, collection: "yours", thumbnail: "/assets/previews/ember.png", custom: true, kind: "artwork", source: "import", created_at: Date.now() };
  },
  applySkin: async () => new Promise<void>((r) => setTimeout(r, 600)),
  revertSkin: async () => new Promise<void>((r) => setTimeout(r, 400)),
  platformInfo: async (): Promise<PlatformInfo> => ({ os: "macos", browse_label: "your Mac", note: "browser preview: nothing is written to disk" }),
  folderIcon: async (): Promise<string> => "/assets/previews/mesh.png",
  deleteSkin: async () => {},
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
    return { id: `user:ai${Date.now()}`, name: name.charAt(0).toUpperCase() + name.slice(1), collection: "yours", thumbnail: "/assets/previews/bubbles.png", custom: true, kind: _req.shape === "folder" ? "folder" : "artwork", source: "ai", created_at: Date.now() };
  },
};
