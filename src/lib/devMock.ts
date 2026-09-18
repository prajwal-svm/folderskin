/**
 * Browser-only stand-in for the Tauri commands so `pnpm dev` in a plain browser shows the
 * real layout with the generated previews (served from assets/previews via public/previews).
 * Never used inside the app: `isTauri()` is true there.
 */
import type { PathInfo, PlatformInfo, Skin, SkinList } from "./tauri";

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

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const mockApi = {
  listSkins: async (): Promise<SkinList> => ({
    skins: IDS.map(([id, name, collection]) => ({ id, name, collection, thumbnail: `/previews/${id}.png`, custom: false })),
    default_thumbnail: "/previews/mesh.png",
  }),
  inspectPath: async (path: string): Promise<PathInfo> => ({ kind: "folder", name: path.split(/[\\/]/).pop() || path, path }),
  importImage: async (path: string): Promise<Skin> => ({ id: `custom:${path}`, name: "your picture", collection: "yours", thumbnail: "/previews/sunset.png", custom: true }),
  applySkin: async () => new Promise<void>((r) => setTimeout(r, 600)),
  revertSkin: async () => new Promise<void>((r) => setTimeout(r, 400)),
  platformInfo: async (): Promise<PlatformInfo> => ({ os: "macos", browse_label: "your Mac", note: "browser preview: nothing is written to disk" }),
  folderIcon: async (): Promise<string> => "/previews/mesh.png",
};
