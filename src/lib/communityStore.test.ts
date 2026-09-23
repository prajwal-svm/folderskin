import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CommunityPack, CommunityQuery, CommunitySearch, PackProgress } from "./tauri";

/** Searches the store has sent, each answered when the test says. */
const asked: { query: CommunityQuery; answer: (r: CommunitySearch) => void; fail: (e: unknown) => void }[] = [];
const added: ((progress: PackProgress) => void)[] = [];

vi.mock("./tauri", () => ({
  api: {
    communitySearch: (query: CommunityQuery) =>
      new Promise<CommunitySearch>((answer, fail) => {
        asked.push({ query, answer, fail });
      }),
    addPack: async (_id: string, onProgress: (p: PackProgress) => void) => {
      added.push(onProgress);
      onProgress({ stage: "download", done: 1, total: 2 });
      return [];
    },
    updatePack: async (_id: string, onProgress: (p: PackProgress) => void) => {
      onProgress({ stage: "save", done: 2, total: 2 });
      return { removed: ["old"], skins: [] };
    },
    communityRefresh: async () => ({ updates: 0, packs: 0 }),
  },
  errorMessage: (e: unknown) => String(e),
}));

const { CommunityStore, PAGE, progressLabel, progressShare } = await import("./communityStore");

function pack(id: string): CommunityPack {
  return { id, name: id, author: "a", license: "CC0-1.0", tags: ["t"], count: 1, bytes: 0, hash: "", preview: "", added: false, update: false };
}

/** An answer of `total` packs, the ones from `offset` on. */
function answer(total: number, offset = 0, prefix = "p"): CommunitySearch {
  const packs = Array.from({ length: Math.max(0, Math.min(PAGE, total - offset)) }, (_, i) => pack(`${prefix}${offset + i}`));
  return { total, all: total, packs, skins: [], hit_packs: [], facets: [], offline: false };
}

const settle = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  asked.length = 0;
  added.length = 0;
});
afterEach(() => vi.useRealTimers());

describe("the Community store", () => {
  it("waits for the typing to stop before it searches", async () => {
    vi.useFakeTimers();
    const store = new CommunityStore(100);
    store.setQuery("n");
    store.setQuery("ne");
    vi.advanceTimersByTime(60);
    store.setQuery("neo");
    expect(asked).toHaveLength(0);
    vi.advanceTimersByTime(100);
    expect(asked.map((a) => a.query.q)).toEqual(["neo"]);
    expect(store.get().query).toBe("neo");
  });

  it("never lets a late answer cover a newer one", async () => {
    const store = new CommunityStore(0);
    void store.search();
    store.setTag("t");
    expect(asked).toHaveLength(2);
    asked[1].answer(answer(3, 0, "new"));
    await settle();
    asked[0].answer(answer(9, 0, "old"));
    await settle();
    const shown = store.get().shown!;
    expect([shown.tag, shown.total, shown.packs[0]?.id]).toEqual(["t", 3, "new0"]);
    expect(store.get().searching).toBe(false);
  });

  it("asks for each page once, as its places are needed, and drops pages of an old answer", async () => {
    const store = new CommunityStore(0);
    void store.search();
    asked[0].answer(answer(1000));
    await settle();
    let shown = store.get().shown!;
    expect(shown.packs).toHaveLength(1000);
    expect(shown.packs[PAGE]).toBeUndefined();

    store.need(PAGE + 5);
    store.need(PAGE + 6);
    store.need(0);
    expect(asked.map((a) => a.query.offset)).toEqual([0, PAGE]);
    asked[1].answer(answer(1000, PAGE));
    await settle();
    shown = store.get().shown!;
    expect(shown.packs[PAGE + 5]?.id).toBe(`p${PAGE + 5}`);

    // A page asked for, then a new search: the page arrives too late to be shown.
    store.need(5 * PAGE);
    void store.search();
    asked[3].answer(answer(10, 0, "q"));
    await settle();
    asked[2].answer(answer(1000, 5 * PAGE));
    await settle();
    expect(store.get().shown!.packs).toHaveLength(10);
  });

  it("marks a pack everywhere it is shown", async () => {
    const store = new CommunityStore(0);
    void store.search();
    const r = answer(2);
    r.hit_packs = [pack("p1")];
    asked[0].answer(r);
    await settle();
    store.open(store.get().shown!.packs[1]!, 0);
    store.mark("p1", true);
    const s = store.get();
    expect([s.shown!.packs[1]?.added, s.shown!.hitPacks[0].added, s.viewing!.pack.added]).toEqual([true, true, true]);
    expect(s.shown!.packs[0]?.added).toBe(false);
  });

  it("keeps adding a pack going, and tells the app, whatever is on screen", async () => {
    const store = new CommunityStore(0);
    const onAdded = vi.fn();
    const toast = vi.fn();
    store.bind({ onAdded, onRemoved: vi.fn(), onShowTag: vi.fn(), toast });
    const heard: [string | null, PackProgress | null][] = [];
    store.subscribe(() => heard.push([store.get().task, store.get().progress]));
    await store.add(pack("p0"));
    expect(heard).toContainEqual(["add", { stage: "download", done: 1, total: 2 }]);
    expect([store.get().busy, store.get().task]).toEqual([null, null]);
    expect(onAdded).toHaveBeenCalledOnce();
    expect(toast.mock.calls[0][0]).toBe("Added 0 skins from p0");

    const onRemoved = vi.fn();
    store.bind({ onAdded, onRemoved, onShowTag: vi.fn(), toast });
    await store.update(pack("p0"));
    expect(heard).toContainEqual(["update", { stage: "save", done: 2, total: 2 }]);
    expect(onRemoved).toHaveBeenCalledWith(["old"]);
    expect(toast.mock.calls[1][0]).toBe("Updated p0");
  });

  it("says how far adding has got", () => {
    expect(progressShare(null)).toBe(0);
    expect(progressShare({ stage: "download", done: 8, total: 16 })).toBeCloseTo(0.425);
    expect(progressShare({ stage: "save", done: 16, total: 16 })).toBe(1);
    expect(progressLabel({ stage: "download", done: 3, total: 16 })).toBe("Downloading 3 of 16");
    expect(progressLabel({ stage: "save", done: 20, total: 16 })).toBe("Saving 16 of 16");
    expect(progressLabel(null)).toBe("Adding");
    expect(progressLabel(null, "Updating")).toBe("Updating");
  });
});
