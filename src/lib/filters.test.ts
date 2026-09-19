import { describe, expect, it } from "vitest";
import {
  activeCount,
  applyFilters,
  facets,
  type FilterContext,
  type Filters,
  matchesQuery,
  NO_FILTERS,
  sortSkins,
  toggleChoice,
} from "./filters";
import type { Palette } from "./palette";
import type { Skin } from "./tauri";

const NOW = new Date(2026, 8, 19, 15, 0).getTime();
const DAY = 24 * 60 * 60 * 1000;

function skin(id: string, over: Partial<Skin> = {}): Skin {
  return { id, name: id, collection: "yours", thumbnail: "", custom: true, tags: [], created_at: NOW - 60_000, ...over };
}

const art = (id: string, over: Partial<Skin> = {}) =>
  skin(id, { source: "community", pack: "classic-art", pack_name: "Classic Art", author: "prajwal-svm", license: "CC0-1.0", ...over });
const pop = (id: string, over: Partial<Skin> = {}) =>
  skin(id, { source: "community", pack: "pop", pack_name: "Scientists - Pop Art", author: "prajwal-svm", license: "CC0-1.0", ...over });

const library: Skin[] = [
  art("mona", { name: "Mona Lisa" }),
  art("starry", { name: "The Starry Night" }),
  pop("curie", { name: "Marie Curie" }),
  skin("beach", { name: "Beach day", source: "import", created_at: NOW - 40 * DAY }),
  skin("dunes", { name: "Dunes", source: "ai", made_with: "OpenAI · GPT Image", idea: "warm desert dunes", created_at: NOW - 3 * DAY }),
];

const palettes = new Map<string, Palette>([
  ["mona", { colours: ["brown", "black"], tone: "dark" }],
  ["starry", { colours: ["blue", "black"], tone: "dark" }],
  ["curie", { colours: ["blue"], tone: "dark" }],
  ["beach", { colours: ["blue", "yellow"], tone: "light" }],
  ["dunes", { colours: ["orange"], tone: "light" }],
]);

const ctx: FilterContext = { favourites: new Set(["starry", "beach"]), palettes, now: NOW };

const ids = (skins: Skin[]) => skins.map((s) => s.id);
const choose = (...picks: [Parameters<typeof toggleChoice>[1], string, boolean?][]): Filters =>
  picks.reduce((f, [facet, value, single]) => toggleChoice(f, facet, value, single), NO_FILTERS);

describe("applyFilters", () => {
  it("lets everything through with nothing chosen", () => {
    expect(applyFilters(library, NO_FILTERS, ctx)).toBe(library);
  });

  it("widens within a facet and narrows across facets", () => {
    expect(ids(applyFilters(library, choose(["colour", "blue"]), ctx))).toEqual(["starry", "curie", "beach"]);
    expect(ids(applyFilters(library, choose(["colour", "blue"], ["colour", "orange"]), ctx))).toEqual(["starry", "curie", "beach", "dunes"]);
    expect(ids(applyFilters(library, choose(["colour", "blue"], ["pack", "classic-art"]), ctx))).toEqual(["starry"]);
  });

  it("keeps only favourites when asked", () => {
    expect(ids(applyFilters(library, { ...NO_FILTERS, favourites: true }, ctx))).toEqual(["starry", "beach"]);
  });

  it("filters by when a skin was added, each window taking in the newer ones", () => {
    expect(ids(applyFilters(library, choose(["added", "today", true]), ctx))).toEqual(["mona", "starry", "curie"]);
    expect(ids(applyFilters(library, choose(["added", "week", true]), ctx))).toEqual(["mona", "starry", "curie", "dunes"]);
    expect(ids(applyFilters(library, choose(["added", "older", true]), ctx))).toEqual(["beach"]);
  });

  it("filters by where a skin came from, what made it, and light or dark", () => {
    expect(ids(applyFilters(library, choose(["source", "import"], ["source", "ai"]), ctx))).toEqual(["beach", "dunes"]);
    expect(ids(applyFilters(library, choose(["model", "OpenAI · GPT Image"]), ctx))).toEqual(["dunes"]);
    expect(ids(applyFilters(library, choose(["tone", "light", true]), ctx))).toEqual(["beach", "dunes"]);
  });
});

describe("facets", () => {
  const byId = (list: ReturnType<typeof facets>) => Object.fromEntries(list.map((f) => [f.id, f]));

  it("offers only facets that can narrow the list", () => {
    const shown = byId(facets(library, NO_FILTERS, ctx));
    expect(Object.keys(shown)).toEqual(["source", "pack", "colour", "tone", "added"]);
    // One author, one licence and one AI model: "From" already tells those skins apart.
    expect(shown.author).toBeUndefined();
    expect(shown.license).toBeUndefined();
    expect(shown.model).toBeUndefined();
  });

  it("counts each choice against the other facets' choices", () => {
    const shown = byId(facets(library, choose(["pack", "classic-art"]), ctx));
    expect(shown.colour.options.map((o) => [o.value, o.count])).toEqual([
      ["blue", 1],
      ["brown", 1],
      ["black", 2],
    ]);
    // Its own choice doesn't narrow its own counts, so the other packs can still be added.
    expect(shown.pack.options.map((o) => [o.label, o.count])).toEqual([
      ["Classic Art", 2],
      ["Scientists - Pop Art", 1],
    ]);
  });

  it("keeps a chosen facet on offer even when nothing matches it any more", () => {
    const filters = choose(["model", "OpenAI · GPT Image"], ["pack", "classic-art"]);
    const shown = byId(facets(library, filters, ctx));
    expect(shown.model.options).toEqual([{ value: "OpenAI · GPT Image", label: "OpenAI · GPT Image", count: 0, swatch: undefined }]);
  });

  it("lists colours in the swatch order, with their swatches", () => {
    const colour = byId(facets(library, NO_FILTERS, ctx)).colour;
    expect(colour.options.map((o) => o.value)).toEqual(["orange", "yellow", "blue", "brown", "black"]);
    expect(colour.options.every((o) => o.swatch?.startsWith("#"))).toBe(true);
  });

  it("leaves out a facet every skin shares", () => {
    const same = [art("a"), art("b")];
    expect(facets(same, NO_FILTERS, { ...ctx, palettes: new Map() }).map((f) => f.id)).toEqual([]);
  });
});

describe("toggleChoice and activeCount", () => {
  it("adds and removes choices, one at a time where only one makes sense", () => {
    const one = toggleChoice(NO_FILTERS, "colour", "blue");
    const two = toggleChoice(one, "colour", "red");
    expect(two.chosen.colour).toEqual(["blue", "red"]);
    expect(toggleChoice(two, "colour", "blue").chosen.colour).toEqual(["red"]);
    expect(toggleChoice(one, "colour", "blue").chosen).toEqual({});

    const today = toggleChoice(NO_FILTERS, "added", "today", true);
    expect(toggleChoice(today, "added", "week", true).chosen.added).toEqual(["week"]);
  });

  it("counts every choice and the favourites switch", () => {
    expect(activeCount(NO_FILTERS)).toBe(0);
    expect(activeCount({ favourites: true, chosen: { colour: ["blue", "red"], pack: ["pop"] } })).toBe(4);
  });
});

describe("sortSkins", () => {
  const list = [skin("b", { name: "Skin 10" }), skin("a", { name: "skin 2" }), skin("c", { name: "Apple" })];

  it("keeps the library's order for newest and turns it round for oldest", () => {
    expect(sortSkins(list, "newest")).toBe(list);
    expect(ids(sortSkins(list, "oldest"))).toEqual(["c", "a", "b"]);
  });

  it("sorts names the way people read them, numbers included", () => {
    expect(ids(sortSkins(list, "az"))).toEqual(["c", "a", "b"]);
    expect(ids(sortSkins(list, "za"))).toEqual(["b", "a", "c"]);
  });
});

describe("matchesQuery", () => {
  it("finds a skin by its name, tags, pack, author or the idea behind it", () => {
    const dunes = library[4];
    expect(matchesQuery(dunes, "DUNES")).toBe(true);
    expect(matchesQuery(dunes, "desert")).toBe(true);
    expect(matchesQuery(dunes, "gpt")).toBe(true);
    expect(matchesQuery(library[2], "pop art")).toBe(true);
    expect(matchesQuery(library[0], "prajwal")).toBe(true);
    expect(matchesQuery(skin("x", { tags: ["wedding"] }), "wed")).toBe(true);
    expect(matchesQuery(library[0], "zebra")).toBe(false);
    expect(matchesQuery(library[0], "  ")).toBe(true);
  });
});
