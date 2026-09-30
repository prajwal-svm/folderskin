import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CollectionPage, CollectionSkin, CollectionSort, CommunityPack, CommunityQuery, CommunitySearch, PackProgress, Skin, UseFrom } from "./tauri";

/** Searches the store has sent, each answered when the test says. */
const asked: { query: CommunityQuery; answer: (r: CommunitySearch) => void; fail: (e: unknown) => void }[] = [];
const added: ((progress: PackProgress) => void)[] = [];
/** What the library holds, as `community_installed` says it. */
let installed: Record<string, string | null> = {};
/** Answers to `community_installed` the test holds back, when it does. */
let heldInstalled: ((answer: Record<string, string | null>) => void)[] | null = null;
/** What `community_pack` answers for a pack id: a pack, null for none, or an Error to fail with. */
let lookups: Record<string, CommunityPack | null | Error> = {};
/** The pack ids `community_pack` was asked for. */
const looked: string[] = [];
/** The pack ids `community_add` was asked for. */
const addedIds: string[] = [];
/** Searches of the official skins the store has sent, each answered when the test says. */
const skinAsks: { q: string; sort: CollectionSort; offset: number; limit: number; answer: (r: CollectionPage) => void; fail: (e: unknown) => void }[] = [];
/** Skins taken on their own, each answered when the test says. */
const uses: { from: UseFrom; sha256: string; answer: (skin: Skin) => void; fail: (e: unknown) => void }[] = [];

vi.mock("./tauri", () => ({
  api: {
    communityPack: async (id: string) => {
      looked.push(id);
      const found = lookups[id] ?? null;
      if (found instanceof Error) throw found.message;
      return found;
    },
    communitySearch: (query: CommunityQuery) =>
      new Promise<CommunitySearch>((answer, fail) => {
        asked.push({ query, answer, fail });
      }),
    addPack: async (id: string, onProgress: (p: PackProgress) => void) => {
      addedIds.push(id);
      added.push(onProgress);
      onProgress({ stage: "download", done: 1, total: 2 });
      return [];
    },
    updatePack: async (_id: string, onProgress: (p: PackProgress) => void) => {
      onProgress({ stage: "save", done: 2, total: 2 });
      return { removed: ["old"], skins: [] };
    },
    communityRefresh: async () => ({ updates: 0, packs: 0 }),
    communityInstalled: () => new Promise((answer) => (heldInstalled ? heldInstalled.push(answer) : answer(installed))),
    removePack: async () => [],
    communityCollection: (q: string, sort: CollectionSort, offset: number, limit: number) =>
      new Promise<CollectionPage>((answer, fail) => {
        skinAsks.push({ q, sort, offset, limit, answer, fail });
      }),
    useCommunitySkin: (from: UseFrom, sha256: string) =>
      new Promise<Skin>((answer, fail) => {
        uses.push({ from, sha256, answer, fail });
      }),
  },
  errorMessage: (e: unknown) => String(e),
}));

const { CommunityStore, PAGE, SKIN_PAGE, collectionCountLine, countLine, progressLabel, progressShare } = await import("./communityStore");

function pack(id: string): CommunityPack {
  return { id, name: id, author: "a", license: "CC0-1.0", tags: ["t"], count: 1, bytes: 0, hash: "", preview: "", added: false, update: false, official: false };
}

/** An answer of `total` packs, the ones from `offset` on. */
function answer(total: number, offset = 0, prefix = "p", generation = "g1"): CommunitySearch {
  const packs = Array.from({ length: Math.max(0, Math.min(PAGE, total - offset)) }, (_, i) => pack(`${prefix}${offset + i}`));
  return { total, all: total, packs, skins: [], collection: [], hit_packs: [], facets: [], last_visit: null, generation };
}

function officialSkin(name: string, i = 0): CollectionSkin {
  return { sha256: `${i}`.padStart(64, "0"), ext: "webp", name, tags: [], thumbnail: "", bytes: 1000, added: 0 };
}

/** A page of `total` official skins, the ones from `offset` on. */
function skinPage(total: number, offset = 0, prefix = "s"): CollectionPage {
  const items = Array.from({ length: Math.max(0, Math.min(SKIN_PAGE, total - offset)) }, (_, i) => officialSkin(`${prefix}${offset + i}`, offset + i));
  return { total, items, license: "MIT" };
}

/** Handlers for `bind`, each a spy, with any of them given. */
const handlers = (given: Partial<Parameters<InstanceType<typeof CommunityStore>["bind"]>[0]> = {}) => ({
  onAdded: vi.fn(),
  onRemoved: vi.fn(),
  onUsed: vi.fn(),
  onShowTag: vi.fn(),
  toast: vi.fn(),
  ...given,
});

const settle = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  asked.length = 0;
  added.length = 0;
  installed = {};
  heldInstalled = null;
  lookups = {};
  looked.length = 0;
  addedIds.length = 0;
  skinAsks.length = 0;
  uses.length = 0;
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

  it("asks for the list again, where it is, when a page comes from a newer catalog", async () => {
    const store = new CommunityStore(0);
    void store.search();
    asked[0].answer(answer(1000));
    await settle();
    store.keepScroll(4000);
    store.need(PAGE);
    // Back online, or Refreshed: the page is another catalog's.
    asked[1].answer(answer(900, PAGE, "n", "g2"));
    await settle();
    expect(store.get().shown!.packs[PAGE]).toBeUndefined();
    expect(asked.map((a) => a.query.offset)).toEqual([0, PAGE, 0]);
    asked[2].answer(answer(900, 0, "n", "g2"));
    await settle();
    const s = store.get();
    expect([s.shown!.total, s.shown!.generation, s.scrollTop]).toEqual([900, "g2", 4000]);
  });

  it("marks the packs it kept against the library when the view comes back", async () => {
    const store = new CommunityStore(0);
    store.start();
    const r = answer(3);
    r.packs[0] = { ...r.packs[0], added: true };
    r.packs[1] = { ...r.packs[1], hash: "new" };
    r.hit_packs = [{ ...r.packs[0] }];
    asked[0].answer(r);
    await settle();
    store.open(store.get().shown!.packs[0]!);

    // Meanwhile p0 was deleted in the library, and p1 added at an older version.
    installed = { p1: "old" };
    store.start();
    await settle();
    const s = store.get();
    const marks = s.shown!.packs.map((p) => [p!.added, p!.update]);
    expect(marks).toEqual([
      [false, false],
      [true, true],
      [false, false],
    ]);
    expect([s.shown!.hitPacks[0].added, s.viewing!.pack.added]).toEqual([false, false]);
    expect(asked).toHaveLength(1);
  });

  it("doesn't lay an older answer about the library over a pack marked since", async () => {
    const store = new CommunityStore(0);
    store.start();
    asked[0].answer(answer(2));
    await settle();
    heldInstalled = [];
    store.start();
    // p0 finishes adding while the app is answering, from before it was added.
    store.mark("p0", true);
    heldInstalled[0]({});
    await settle();
    expect(heldInstalled).toHaveLength(2);
    heldInstalled[1]({ p0: "" });
    await settle();
    expect(store.get().shown!.packs[0]?.added).toBe(true);
  });

  it("works on one pack at a time, and says which to wait for", async () => {
    const store = new CommunityStore(0);
    const toast = vi.fn();
    store.bind(handlers({ toast }));
    const adding = store.add({ ...pack("p0"), name: "Colours" });
    await store.remove(pack("p1"));
    expect(toast).toHaveBeenCalledWith("Colours is still being added. Try again once it's done.");
    await adding;
    await store.remove(pack("p1"));
    expect(toast).toHaveBeenLastCalledWith("Removed p1", { tone: "ok" });
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
    store.bind(handlers({ onAdded, toast }));
    const heard: [string | null, PackProgress | null][] = [];
    store.subscribe(() => heard.push([store.get().task, store.get().progress]));
    await store.add(pack("p0"));
    expect(heard).toContainEqual(["add", { stage: "download", done: 1, total: 2 }]);
    expect([store.get().busy, store.get().task]).toEqual([null, null]);
    expect(onAdded).toHaveBeenCalledOnce();
    expect(toast.mock.calls[0][0]).toBe("Added 0 skins from p0");

    const onRemoved = vi.fn();
    store.bind(handlers({ onAdded, onRemoved, toast }));
    await store.update(pack("p0"));
    expect(heard).toContainEqual(["update", { stage: "save", done: 2, total: 2 }]);
    expect(onRemoved).toHaveBeenCalledWith(["old"]);
    expect(toast.mock.calls[1][0]).toBe("Updated p0");
  });

  it("opens the pack an install link names and adds it as its Add button does", async () => {
    const store = new CommunityStore(0);
    const onAdded = vi.fn();
    const toast = vi.fn();
    lookups = { colours: { ...pack("colours"), name: "Colours", official: true } };
    // Before Community has ever been open: it waits for the view's handlers.
    store.install("colours");
    await settle();
    expect(looked).toEqual([]);
    const heard: (string | null)[] = [];
    store.subscribe(() => heard.push(store.get().task));
    store.bind(handlers({ onAdded, toast }));
    await vi.waitFor(() => expect(toast).toHaveBeenCalledOnce());
    expect(store.get().busy).toBeNull();
    expect(looked).toEqual(["colours"]);
    expect(addedIds).toEqual(["colours"]);
    expect(store.get().viewing?.pack.id).toBe("colours");
    expect(heard).toContain("add");
    expect(onAdded).toHaveBeenCalledOnce();
    expect(toast).toHaveBeenCalledWith("Added 0 skins from Colours", expect.objectContaining({ tone: "ok" }));

    // Binding again, as every draw of the view does, doesn't add it twice.
    store.bind(handlers({ onAdded, toast }));
    await settle();
    expect(addedIds).toEqual(["colours"]);
  });

  it("says a linked pack is there already, and adds nothing", async () => {
    const store = new CommunityStore(0);
    const toast = vi.fn();
    store.bind(handlers({ toast }));
    lookups = {
      colours: { ...pack("colours"), name: "Colours", added: true },
      greek: { ...pack("greek"), name: "Greek Art", added: true, update: true },
    };
    store.install("colours");
    await vi.waitFor(() => expect(toast).toHaveBeenCalledTimes(1));
    expect(toast).toHaveBeenLastCalledWith("Colours is in your library already", expect.objectContaining({ tone: "ok" }));
    expect(store.get().viewing?.pack.id).toBe("colours");
    store.install("greek");
    await vi.waitFor(() => expect(toast).toHaveBeenCalledTimes(2));
    expect(toast).toHaveBeenLastCalledWith(
      "Greek Art is in your library already, and Update gets its newer version",
      expect.objectContaining({ tone: "ok" }),
    );
    expect(addedIds).toEqual([]);
  });

  it("says what went wrong with a linked pack, and what to do", async () => {
    const store = new CommunityStore(0);
    const toast = vi.fn();
    store.bind(handlers({ toast }));
    store.install("gone-pack");
    await vi.waitFor(() => expect(toast).toHaveBeenCalledTimes(1));
    const [said, opts] = toast.mock.calls[0];
    expect(said).toBe("Couldn't add “gone-pack”: there's no pack by that name in Community. Search for it there, as it may have been renamed.");
    expect(said).not.toMatch(/sorry|apolog/i);
    expect(opts).toEqual({ tone: "danger" });

    lookups = { colours: new Error("couldn't reach GitHub. Check your connection and try again") };
    store.install("colours");
    await vi.waitFor(() => expect(toast).toHaveBeenCalledTimes(2));
    expect(toast).toHaveBeenLastCalledWith("Couldn't add “colours”: couldn't reach GitHub. Check your connection and try again", { tone: "danger" });
    expect(store.get().viewing).toBeNull();
    expect(addedIds).toEqual([]);
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

  it("counts what the list holds in words that agree", () => {
    const shown = (total: number, q = "", tag = "") =>
      ({ q, tag, sort: "best", id: 1, total, all: 10004, packs: [], skins: [], official: [], hitPacks: [], facets: [], lastVisit: null, generation: "g" }) as Parameters<typeof countLine>[0];
    expect(countLine(shown(10004), null)).toBe("10,004 packs");
    expect(countLine(shown(1, "greek"), null)).toBe("1 pack matches “greek”");
    expect(countLine(shown(23, "koi"), null)).toBe("23 packs match “koi”");
    expect(countLine(shown(1, "pop", "art"), null)).toBe("1 pack matches “pop” tagged Art");
    expect(countLine(shown(0, "zzz"), null)).toBe("0 packs match “zzz”");
    expect(countLine(null, null)).toBe("");
  });
});

describe("the official skins in the Community store", () => {
  it("counts them once for their tab, keeping their first page for when it's picked", async () => {
    const store = new CommunityStore(0);
    store.start();
    void store.warm();
    expect(skinAsks.map((a) => [a.q, a.sort, a.offset, a.limit])).toEqual([["", "newest", 0, SKIN_PAGE]]);
    skinAsks[0].answer(skinPage(587));
    await settle();
    const s = store.get();
    expect(s.collectionSize).toBe(587);
    expect([s.collection?.q, s.collection?.total, s.collection?.skins[0]?.name]).toEqual(["", 587, "s0"]);
    // Picked with nothing typed, that page is the answer: nothing is asked again.
    store.showOfficial();
    expect(skinAsks).toHaveLength(1);
    expect(store.get().tab).toBe("official");
  });

  it("searches only the list on show as the words change, and the other when its tab is picked", async () => {
    vi.useFakeTimers();
    const store = new CommunityStore(100);
    store.showOfficial();
    expect(skinAsks.map((a) => a.q)).toEqual([""]);
    store.setQuery("gir");
    vi.advanceTimersByTime(100);
    expect(skinAsks.map((a) => a.q)).toEqual(["", "gir"]);
    expect(asked).toHaveLength(0);
    skinAsks[1].answer(skinPage(3, 0, "giraffe"));
    await vi.advanceTimersByTimeAsync(0);
    expect(collectionCountLine(store.get().collection, null)).toBe("3 official skins match “gir”");

    // The packs, for the same words, and back: the official answer still answers them.
    store.setTag("");
    expect(asked.map((a) => [a.query.q, a.query.tag])).toEqual([["gir", ""]]);
    store.showOfficial();
    expect(skinAsks).toHaveLength(2);
    // A new order is a new search.
    store.setCollectionSort("name");
    expect(skinAsks.map((a) => [a.q, a.sort])).toEqual([
      ["", "newest"],
      ["gir", "newest"],
      ["gir", "name"],
    ]);
  });

  it("finds the packs as they were, scrolled where they were, back from the official skins", async () => {
    const store = new CommunityStore(0);
    void store.search();
    asked[0].answer(answer(1000));
    await settle();
    store.keepScroll(4000);
    store.showOfficial();
    skinAsks[0].answer(skinPage(500));
    await settle();
    store.keepScroll(900);
    store.setTag("");
    expect(asked).toHaveLength(1);
    const s = store.get();
    expect([s.tab, s.scrollTop, s.collectionScroll]).toEqual(["packs", 4000, 900]);
    // A tag of their own is a search of the packs, as ever.
    store.setTag("t");
    expect(asked).toHaveLength(2);
  });

  it("asks for each page of them once, as its places are needed, and drops pages of an old answer", async () => {
    const store = new CommunityStore(0);
    store.showOfficial();
    skinAsks[0].answer(skinPage(1000));
    await settle();
    expect(store.get().collection!.skins).toHaveLength(1000);
    store.needSkin(SKIN_PAGE + 3);
    store.needSkin(SKIN_PAGE + 4);
    store.needSkin(0);
    expect(skinAsks.map((a) => a.offset)).toEqual([0, SKIN_PAGE]);
    skinAsks[1].answer(skinPage(1000, SKIN_PAGE));
    await settle();
    expect(store.get().collection!.skins[SKIN_PAGE + 3]?.name).toBe(`s${SKIN_PAGE + 3}`);

    store.needSkin(5 * SKIN_PAGE);
    store.setCollectionSort("name");
    skinAsks[3].answer(skinPage(10, 0, "n"));
    await settle();
    skinAsks[2].answer(skinPage(1000, 5 * SKIN_PAGE));
    await settle();
    expect(store.get().collection!.skins).toHaveLength(10);
  });

  it("brings the words' official skins along with a search of the packs", async () => {
    const store = new CommunityStore(0);
    void store.search();
    asked[0].answer({ ...answer(2), collection: [officialSkin("Giraffe cola")] });
    await settle();
    expect(store.get().shown!.official.map((s) => s.name)).toEqual(["Giraffe cola"]);
  });

  it("hands a skin taken on its own to the app, and closes the viewers behind it, one at a time", async () => {
    const store = new CommunityStore(0);
    const onUsed = vi.fn();
    const toast = vi.fn();
    store.bind(handlers({ onUsed, toast }));
    const giraffe = officialSkin("Giraffe cola", 7);
    store.openSkin(giraffe);
    const using = store.use({ kind: "collection" }, giraffe.sha256, giraffe.name);
    expect(store.get().using).toBe(giraffe.sha256);
    // Another while it's being saved does nothing.
    void store.use({ kind: "pack", id: "p0", hash: "h" }, "other", "Other");
    expect(uses.map((u) => u.sha256)).toEqual([giraffe.sha256]);
    const skin = { id: "user:1", name: "Giraffe cola", collection: "yours", thumbnail: "", custom: true, tags: [] } as Skin;
    uses[0].answer(skin);
    await using;
    const s = store.get();
    expect([s.using, s.viewingSkin, s.viewing]).toEqual([null, null, null]);
    expect(onUsed).toHaveBeenCalledWith(skin);
    // The app says it's in the library: the store has nothing to add.
    expect(toast).not.toHaveBeenCalled();

    // From a pack, which is not marked added.
    store.open(pack("p0"));
    const fromPack = store.use({ kind: "pack", id: "p0", hash: "h0" }, "abc", "Mona Lisa");
    expect(uses[1].from).toEqual({ kind: "pack", id: "p0", hash: "h0" });
    uses[1].answer({ ...skin, id: "user:2" });
    await fromPack;
    expect(store.get().viewing).toBeNull();
    expect(addedIds).toEqual([]);
  });

  it("says why a skin couldn't be taken, and leaves its viewer open", async () => {
    const store = new CommunityStore(0);
    const onUsed = vi.fn();
    const toast = vi.fn();
    store.bind(handlers({ onUsed, toast }));
    const giraffe = officialSkin("Giraffe cola");
    store.openSkin(giraffe);
    const using = store.use({ kind: "collection" }, giraffe.sha256, giraffe.name);
    uses[0].fail("that skin isn't listed any more. Try Refresh");
    await using;
    expect(toast).toHaveBeenCalledWith("Couldn't add Giraffe cola: that skin isn't listed any more. Try Refresh", { tone: "danger" });
    expect(onUsed).not.toHaveBeenCalled();
    expect([store.get().using, store.get().viewingSkin?.name]).toEqual([null, "Giraffe cola"]);
  });

  it("counts them in words that agree", () => {
    const shown = (total: number, q = "") => ({ q, sort: "newest" as const, id: 1, total, skins: [] });
    expect(collectionCountLine(shown(587), null)).toBe("587 official skins");
    expect(collectionCountLine(shown(1), null)).toBe("1 official skin");
    expect(collectionCountLine(shown(1, "cat"), null)).toBe("1 official skin matches “cat”");
    expect(collectionCountLine(shown(12, "cat"), null)).toBe("12 official skins match “cat”");
    expect(collectionCountLine(shown(12, "cat"), "you're offline")).toBe("Couldn't search: you're offline");
    expect(collectionCountLine(null, null)).toBe("");
  });
});
