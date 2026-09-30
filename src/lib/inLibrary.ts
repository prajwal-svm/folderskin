/**
 * Which community skins are in the library already, for the ticks Community shows on them. The
 * app says what id each skin would have in the library (`skin_id`), and the library's own list
 * says which ids it holds, so the ticks follow the library as it changes: a skin used, deleted or
 * added with its pack is ticked or not at once, with nothing asked of the app.
 *
 * A skin taken on its own ("Use") remembers the pack it came from (`from_pack`), so a pack's card
 * can say how many of its skins are in the library without its skins being listed.
 */
import type { CommunityPack, PackSkinPreview, Skin } from "./tauri";

/** The library, as Community looks at it. */
export type LibraryIndex = {
  /** Every skin's id. */
  ids: ReadonlySet<string>;
  /** How many skins were taken on their own from each pack that isn't added, by the pack's id. */
  fromPack: ReadonlyMap<string, number>;
};

export const EMPTY_LIBRARY: LibraryIndex = { ids: new Set(), fromPack: new Map() };

/** The index of `skins`, the library's. */
export function libraryIndex(skins: readonly Pick<Skin, "id" | "pack" | "from_pack">[]): LibraryIndex {
  const ids = new Set<string>();
  const fromPack = new Map<string, number>();
  for (const skin of skins) {
    ids.add(skin.id);
    // One of an added pack's own skins is the pack's, however it got there.
    if (skin.from_pack && !skin.pack) fromPack.set(skin.from_pack, (fromPack.get(skin.from_pack) ?? 0) + 1);
  }
  return { ids, fromPack };
}

/** Whether the skin whose library id is `skinId` is in the library; false when the id isn't known. */
export function inLibrary(index: LibraryIndex, skinId: string | null | undefined): boolean {
  return !!skinId && index.ids.has(skinId);
}

/**
 * How many of `pack`'s skins are in the library while the pack itself isn't: 0 for an added pack,
 * which says so on its own. With the pack's `skins` listed and every one's id known, they are
 * counted by id, which finds one that came from elsewhere too (the same picture official, say);
 * otherwise by the skins taken from the pack on their own. Never more than the pack has.
 */
export function packInLibrary(index: LibraryIndex, pack: Pick<CommunityPack, "id" | "added" | "count">, skins?: readonly PackSkinPreview[] | null): number {
  if (pack.added) return 0;
  const byId = skins && skins.length > 0 && skins.every((s) => s.skin_id) ? new Set(skins.filter((s) => inLibrary(index, s.skin_id)).map((s) => s.skin_id)).size : null;
  return Math.min(byId ?? index.fromPack.get(pack.id) ?? 0, pack.count);
}
