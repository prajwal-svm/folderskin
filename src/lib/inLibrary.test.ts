import { describe, expect, it } from "vitest";
import { EMPTY_LIBRARY, inLibrary, libraryIndex, packInLibrary } from "./inLibrary";
import type { PackSkinPreview } from "./tauri";

const skin = (id: string, pack: string | null = null, from_pack: string | null = null) => ({ id, pack, from_pack });
const preview = (skin_id: string | null): PackSkinPreview => ({ name: skin_id ?? "drive", tags: [], thumbnail: "", sha256: "a".repeat(64), ext: "webp", skin_id });
const blues = { id: "blues", added: false, count: 4 };

describe("what's in the library", () => {
  it("knows a skin by the id the app says it would have", () => {
    const index = libraryIndex([skin("user:aaa"), skin("user:bbb", "reds")]);
    expect(inLibrary(index, "user:aaa")).toBe(true);
    expect(inLibrary(index, "user:bbb")).toBe(true);
    expect(inLibrary(index, "user:ccc")).toBe(false);
    // A skin of a pack of drives, whose id can't be known without its picture.
    expect(inLibrary(index, null)).toBe(false);
    expect(inLibrary(index, undefined)).toBe(false);
    expect(inLibrary(EMPTY_LIBRARY, "user:aaa")).toBe(false);
  });

  it("counts the skins taken from a pack on their own, and not an added pack's own", () => {
    const index = libraryIndex([skin("user:1", null, "blues"), skin("user:2", null, "blues"), skin("user:3", null, "reds"), skin("user:4", "greens", "greens"), skin("user:5")]);
    expect(index.fromPack.get("blues")).toBe(2);
    expect(index.fromPack.get("reds")).toBe(1);
    expect(index.fromPack.has("greens")).toBe(false);
    expect(packInLibrary(index, blues)).toBe(2);
    expect(packInLibrary(index, { ...blues, id: "whites" })).toBe(0);
  });

  it("says nothing of an added pack's skins: the pack says it's added", () => {
    const index = libraryIndex([skin("user:1", null, "blues")]);
    expect(packInLibrary(index, { ...blues, added: true })).toBe(0);
  });

  it("counts a listed pack's skins by id, which finds the ones that came from elsewhere too", () => {
    // user:2 is the same picture, used from the official collection.
    const index = libraryIndex([skin("user:1", null, "blues"), skin("user:2", null, null)]);
    const skins = [preview("user:1"), preview("user:2"), preview("user:3"), preview("user:4")];
    expect(packInLibrary(index, blues, skins)).toBe(2);
    // Deleted from the library: gone from the count at once.
    expect(packInLibrary(libraryIndex([skin("user:2")]), blues, skins)).toBe(1);
  });

  it("falls back on the skins taken from it when some ids can't be known", () => {
    const index = libraryIndex([skin("user:1", null, "plain-drives")]);
    const drives = { id: "plain-drives", added: false, count: 3 };
    expect(packInLibrary(index, drives, [preview(null), preview(null), preview(null)])).toBe(1);
    expect(packInLibrary(index, drives, [])).toBe(1);
  });

  it("never counts more than the pack has", () => {
    const index = libraryIndex([skin("user:1", null, "tiny"), skin("user:2", null, "tiny")]);
    expect(packInLibrary(index, { id: "tiny", added: false, count: 1 })).toBe(1);
  });
});
