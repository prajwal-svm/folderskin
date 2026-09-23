import { describe, expect, it } from "vitest";
import { madeUpPacks, MockCatalog, type MockPack, words } from "./mockCommunity";

/** The packs crates/folderskin-catalog tests with, so the two searches can be read side by side. */
function pack(id: string, name: string, author: string, tags: string[], added: number, skins: string[]): MockPack {
  return { id, name, author, license: "CC0-1.0", tags, count: skins.length, bytes: 0, hash: "", added, skins, preview: "" };
}

const SAMPLE = [
  pack("classic-art", "Classic Art", "prajwal-svm", ["classic art", "painting"], 300, ["Mona Lisa", "The Starry Night", "View of Toledo", "Composition VIII"]),
  pack("colours", "Colours", "prajwal-svm", ["colour"], 100, ["Blue", "Orange", "Purple", "Green"]),
  pack("greek-art", "Greek Art", "someone", ["classic art", "sculpture"], 200, ["Parthenon", "Poseidon", "Marble youth"]),
  pack("night-prints", "Night prints", "hokusai-fan", ["woodblock", "night"], 400, ["Starling", "Night heron", "Moon over Edo"]),
  pack("cafe-noir", "Café noir", "barista", ["coffee"], 50, ["Espresso"]),
];

const catalog = (featured: string[] = []) => new MockCatalog(SAMPLE, featured);
const find = (c: MockCatalog, q: string, more: Partial<Parameters<MockCatalog["search"]>[0]> = {}) =>
  c.search({ q, tag: "", sort: "best", offset: 0, limit: 50, ...more });
const ids = (r: { packs: MockPack[] }) => r.packs.map((p) => p.id);

describe("the preview's search follows the app's", () => {
  it("splits text into words the way the index does", () => {
    expect(words("Café noir, ukiyo-e!")).toEqual(["cafe", "noir", "ukiyo", "e"]);
    expect(words("  ")).toEqual([]);
  });

  it("matches every word as the start of a word in a name, author, tag or skin", () => {
    const c = catalog();
    expect(ids(find(c, "star")).sort()).toEqual(["classic-art", "night-prints"]);
    expect(ids(find(c, "clas")).sort()).toEqual(["classic-art", "greek-art"]);
    expect(ids(find(c, "prajwal")).sort()).toEqual(["classic-art", "colours"]);
    expect(ids(find(c, "starry toledo"))).toEqual(["classic-art"]);
    expect(find(c, "starry parthenon").packs).toEqual([]);
    expect(ids(find(c, "CAFÉ"))).toEqual(["cafe-noir"]);
    expect(find(c, "").total).toBe(5);
  });

  it("ranks a name above a skin", () => {
    expect(ids(find(catalog(), "night"))).toEqual(["night-prints", "classic-art"]);
  });

  it("counts tags over the words alone, and narrows the list by one", () => {
    const c = catalog();
    const all = find(c, "");
    expect(all.facets[0]).toEqual({ tag: "classic art", count: 2 });
    expect(all.facets).toHaveLength(7);
    const r = find(c, "art", { tag: "sculpture" });
    expect(ids(r)).toEqual(["greek-art"]);
    expect([r.total, r.all]).toEqual([1, 2]);
    expect(r.facets).toEqual([
      { tag: "classic art", count: 2 },
      { tag: "painting", count: 1 },
      { tag: "sculpture", count: 1 },
    ]);
  });

  it("pages in the order asked for, featured first when nothing is typed", () => {
    const c = catalog(["colours", "gone", "greek-art"]);
    expect(ids(find(c, "", { sort: "newest", offset: 0, limit: 2 }))).toEqual(["night-prints", "classic-art"]);
    expect(ids(find(c, "", { sort: "newest", offset: 4, limit: 2 }))).toEqual(["cafe-noir"]);
    expect(ids(find(c, "", { sort: "name", limit: 2 }))).toEqual(["cafe-noir", "classic-art"]);
    expect(ids(find(c, "", { sort: "skins", limit: 1 }))).toEqual(["classic-art"]);
    expect(ids(find(c, "", { limit: 3 }))).toEqual(["colours", "greek-art", "night-prints"]);
    expect(ids(find(c, "", { tag: "classic art" }))).toEqual(["greek-art", "classic-art"]);
  });

  it("finds skins by name, whole words first", () => {
    const c = catalog();
    const hits = find(c, "night").skins.map((h) => [h.pack.id, h.name, h.index]);
    expect(hits).toEqual([
      ["classic-art", "The Starry Night", 1],
      ["night-prints", "Night heron", 1],
    ]);
    expect(find(c, "star").skins.map((h) => h.name)).toEqual(["The Starry Night", "Starling"]);
    expect(find(c, "night", { tag: "woodblock" }).skins).toHaveLength(1);
    expect(find(c, "").skins).toEqual([]);
  });
});

describe("made-up packs", () => {
  it("are the same packs for the same number, each with its own id", () => {
    const a = madeUpPacks(2000, ["/p.png"]);
    expect(a).toEqual(madeUpPacks(2000, ["/p.png"]));
    expect(new Set(a.map((p) => p.id)).size).toBe(2000);
    expect(new Set(a.map((p) => p.name)).size).toBe(2000);
    for (const p of a) {
      expect(p.id).toMatch(/^[a-z0-9]+(-[a-z0-9]+)*$/);
      expect(p.id.length).toBeLessThanOrEqual(40);
      expect(p.skins).toHaveLength(p.count);
      expect(p.tags.length).toBeGreaterThan(0);
    }
  });

  it("search in a few milliseconds at ten thousand", () => {
    const c = new MockCatalog(madeUpPacks(10_000, ["/p.png"]));
    const started = performance.now();
    for (const q of ["n", "ne", "neo", "neon", "neon k"]) c.search({ q, tag: "", sort: "best", offset: 0, limit: 60 });
    // Generous: it only has to leave room for typing in the browser preview.
    expect((performance.now() - started) / 5).toBeLessThan(100);
  });
});
