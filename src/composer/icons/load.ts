/**
 * Getting icon packs: the built-in one from the app itself (a separate chunk, so it costs nothing
 * until the icon library opens), the others from what's been downloaded. Each is read and indexed
 * once a session.
 */
import { api, type IconPackProgress } from "../../lib/tauri";
import { ICON_PACKS, ICON_RELEASE, type IconPackInfo } from "./catalog";
import { indexPack, parsePack, type IconIndex, type IconPack } from "./index";

export type LoadedPack = { pack: IconPack; index: IconIndex };

const loaded = new Map<string, Promise<LoadedPack>>();

export const BUILTIN_PACK = ICON_PACKS.find((p) => p.builtin)?.id ?? "lucide";

export function packInfo(id: string): IconPackInfo | undefined {
  return ICON_PACKS.find((p) => p.id === id);
}

async function read(id: string): Promise<LoadedPack> {
  const raw: unknown = id === BUILTIN_PACK ? (await import("./lucide.json")).default : await api.iconPackRead(id);
  const pack = parsePack(raw);
  if (!pack) throw new Error("that icon pack couldn't be read. Remove it and download it again");
  return { pack, index: indexPack(pack) };
}

/** Pack `id`, read and indexed. A failed read is forgotten, so trying again reads it again. */
export function loadPack(id: string): Promise<LoadedPack> {
  let p = loaded.get(id);
  if (!p) {
    p = read(id);
    loaded.set(id, p);
    p.catch(() => loaded.delete(id));
  }
  return p;
}

/** Downloads a pack the catalog lists, checked against the hash the catalog carries. */
export async function downloadPack(info: IconPackInfo, onProgress: (p: IconPackProgress) => void): Promise<void> {
  await api.iconPackDownload(info.id, ICON_RELEASE, info.sha256, info.bytes, onProgress);
  loaded.delete(info.id);
}

export async function removePack(id: string): Promise<void> {
  await api.iconPackRemove(id);
  loaded.delete(id);
}

/** Which packs can be used now: the built-in one and every one downloaded with the catalog's hash. */
export async function availablePacks(): Promise<Set<string>> {
  const installed = await api.iconPacksInstalled().catch(() => []);
  const ok = new Set<string>([BUILTIN_PACK]);
  for (const p of installed) {
    const info = packInfo(p.id);
    // An older download of a pack that has since changed isn't the one this version expects.
    if (info && (p.sha256 === "" || p.sha256 === info.sha256)) ok.add(p.id);
  }
  return ok;
}
