/**
 * The community catalog for the browser preview: made-up packs in any number (`?packs=10000`),
 * made-up official skins (`?collection=600`), and a search over them that follows the app's
 * (crates/folderskin-catalog/src/search.rs) rule for rule, so the preview and the end-to-end tests
 * see what the app would. Nothing here runs in the app, and nothing runs at all until the preview
 * first asks for packs.
 */
import type { CollectionSort, CommunitySort } from "./tauri";

/** A pack as the preview's catalog holds it. */
export type MockPack = {
  id: string;
  name: string;
  author: string;
  license: string;
  tags: string[];
  count: number;
  bytes: number;
  hash: string;
  /** When it was published, in Unix seconds; 0 when nobody knows. */
  added: number;
  /** Its skins' names, in the pack's order. */
  skins: string[];
  /** Its preview strip. */
  preview: string;
  /** True for a pack of drives. */
  drives?: boolean;
};

export type MockHit = { pack: MockPack; name: string; index: number };

export type MockResults = {
  total: number;
  all: number;
  packs: MockPack[];
  skins: MockHit[];
  facets: { tag: string; count: number }[];
};

export type MockQuery = { q: string; tag: string; sort: CommunitySort; offset: number; limit: number };

/** The same limits as the app's catalog. */
export const MAX_PAGE = 200;
export const SKIN_HITS = 12;
export const MAX_FACETS = 200;
const MAX_WORDS = 8;

/**
 * Lower case, accents off, split on anything but letters, digits and private-use characters: how
 * the index sees text. Only the Latin accents come off; every other mark, such as the vowel
 * signs of Hindi or Thai, ends a word there, so "हिन्दी" is ह, न and द, as it is to the app.
 */
export function words(text: string): string[] {
  return text
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .normalize("NFC")
    .toLowerCase()
    .split(/[^\p{L}\p{N}\p{Co}]+/u)
    .filter(Boolean);
}

/** One pack's words, with how much a match in each counts, as bm25's column weights do. */
type Indexed = { pack: MockPack; order: number; fields: [string[], number][]; skinWords: string[][] };

/** Name, then tags, author and skin names: the weights search.rs gives bm25. */
const WEIGHTS = { name: 10, author: 3, tags: 4, skins: 1 };

export class MockCatalog {
  private indexed: Indexed[];
  private featured: string[];

  constructor(packs: MockPack[], featured: string[] = []) {
    // The catalog keeps packs in id order, and ties in every sort fall back to it.
    const sorted = [...packs].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
    this.indexed = sorted.map((pack, order) => ({
      pack,
      order,
      fields: [
        [words(pack.name), WEIGHTS.name],
        [words(pack.author), WEIGHTS.author],
        [words(pack.tags.join(" ")), WEIGHTS.tags],
        [pack.skins.flatMap(words), WEIGHTS.skins],
      ],
      skinWords: pack.skins.map(words),
    }));
    this.featured = featured;
  }

  get packs(): MockPack[] {
    return this.indexed.map((i) => i.pack);
  }

  find(id: string): MockPack | undefined {
    return this.indexed.find((i) => i.pack.id === id)?.pack;
  }

  search({ q, tag, sort, offset, limit }: MockQuery): MockResults {
    const typed = words(q).slice(0, MAX_WORDS);
    const page = Math.min(Math.max(limit, 1), MAX_PAGE);
    // Every word has to start a word of the pack somewhere; the score is how much it counts where.
    const scored: { item: Indexed; score: number }[] = [];
    for (const item of this.indexed) {
      let score = 0;
      let every = true;
      for (const w of typed) {
        let best = 0;
        for (const [ws, weight] of item.fields) if (weight > best && ws.some((x) => x.startsWith(w))) best = weight;
        if (!best) {
          every = false;
          break;
        }
        score += best;
      }
      if (every) scored.push({ item, score });
    }
    const tagged = tag ? scored.filter((s) => s.item.pack.tags.includes(tag)) : scored;

    const facets = new Map<string, number>();
    for (const { item } of scored) for (const t of item.pack.tags) facets.set(t, (facets.get(t) ?? 0) + 1);

    let ordered: MockPack[];
    if (sort === "best" && typed.length) {
      ordered = [...tagged].sort((a, b) => b.score - a.score || a.item.order - b.item.order).map((s) => s.item.pack);
    } else if (sort === "best") {
      const first = this.featured.map((id) => tagged.find((s) => s.item.pack.id === id)?.item).filter((i): i is Indexed => !!i);
      const rest = tagged.map((s) => s.item).filter((i) => !first.includes(i));
      ordered = [...first, ...rest.sort(byNewest)].map((i) => i.pack);
    } else {
      const by = sort === "newest" ? byNewest : sort === "name" ? byName : bySkins;
      ordered = tagged.map((s) => s.item).sort(by).map((i) => i.pack);
    }

    return {
      total: tagged.length,
      all: scored.length,
      packs: ordered.slice(offset, offset + page),
      skins: typed.length ? this.skinHits(typed, tag) : [],
      facets: [...facets]
        .map(([t, count]) => ({ tag: t, count }))
        .sort((a, b) => b.count - a.count || (a.tag < b.tag ? -1 : 1))
        .slice(0, MAX_FACETS),
    };
  }

  /** Skins whose names hold every word whole first, then those the words only start, each in the catalog's order. */
  private skinHits(typed: string[], tag: string): MockHit[] {
    const whole: MockHit[] = [];
    const starts: MockHit[] = [];
    for (const item of this.indexed) {
      if (tag && !item.pack.tags.includes(tag)) continue;
      item.skinWords.forEach((ws, index) => {
        if (!typed.every((w) => ws.some((x) => x.startsWith(w)))) return;
        const hit = { pack: item.pack, name: item.pack.skins[index], index };
        if (typed.every((w) => ws.includes(w))) {
          if (whole.length < SKIN_HITS) whole.push(hit);
        } else if (starts.length < SKIN_HITS) starts.push(hit);
      });
      if (whole.length >= SKIN_HITS) break;
    }
    return [...whole, ...starts].slice(0, SKIN_HITS);
  }
}

const byNewest = (a: Indexed, b: Indexed) => b.pack.added - a.pack.added || a.order - b.order;
const byName = (a: Indexed, b: Indexed) => {
  const x = a.pack.name.toLowerCase();
  const y = b.pack.name.toLowerCase();
  return x < y ? -1 : x > y ? 1 : a.order - b.order;
};
const bySkins = (a: Indexed, b: Indexed) => b.pack.count - a.pack.count || a.order - b.order;

// ---------- made-up packs ----------

const ADJECTIVES = [
  "Neon", "Quiet", "Golden", "Velvet", "Paper", "Midnight", "Sunlit", "Rusty", "Pastel", "Electric", "Frozen",
  "Wild", "Tiny", "Grand", "Misty", "Lucky", "Cosmic", "Retro", "Faded", "Bright", "Hidden", "Soft", "Stormy",
  "Crimson", "Amber", "Jade", "Ivory", "Copper", "Silver", "Dusky", "Lunar", "Solar", "Coral", "Mossy", "Café",
];
const NOUNS = [
  "Koi", "Harbour", "Gardens", "Foxes", "Lanterns", "Circuits", "Temples", "Waves", "Owls", "Planets", "Forests",
  "Robots", "Mountains", "Cities", "Cats", "Dragons", "Deserts", "Ships", "Moths", "Orchards", "Glaciers", "Arcades",
  "Lighthouses", "Meadows", "Comets", "Tigers", "Kites", "Reefs", "Canyons", "Trains", "Teacups", "Castles",
];
const THINGS = [
  "Heron", "Maple", "Anchor", "Comet", "Lotus", "Fern", "Pebble", "Ember", "Willow", "Cedar", "Harbour", "Lantern",
  "Sparrow", "Tide", "Cloud", "Thistle", "Nebula", "Circuit", "Garnet", "Sable", "Orchid", "Quartz", "Juniper",
  "Falcon", "Marble", "Poppy", "Saffron", "Indigo", "Cobalt", "Basil", "Birch", "Kestrel", "Mosaic", "Origami",
];
const TAGS = [
  "anime", "retro", "nature", "space", "minimal", "pixel", "night", "animals", "architecture", "abstract", "photo",
  "painting", "cute", "dark", "pastel", "neon", "ocean", "food", "games", "music", "sport", "travel", "flowers",
  "tech", "vintage", "woodblock", "watercolour", "3d", "gradient", "classic art", "sci-fi", "fantasy", "seasons",
  "cities", "botanical", "geometric", "handmade", "ink", "glass", "metal",
];
const AUTHORS = [
  "prajwal-svm", "octocat", "pixel-pusher", "mika", "lena-draws", "tomo", "zed", "art-by-ana", "kofi", "yuki",
  "folder-fan", "hana-k", "rio", "studio-mint", "quietfox", "bytebrush", "leafy", "nova-art", "marek", "ines-p",
];
const LICENSES = ["CC0-1.0", "CC0-1.0", "CC0-1.0", "CC-BY-4.0", "MIT"];

/** A small, fast generator with a fixed start, so the same `?packs=N` is the same packs. */
function random(seed: number) {
  let s = seed >>> 0 || 1;
  return (n: number) => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return (s >>> 0) % n;
  };
}

/**
 * `n` made-up packs, as varied as real ones: two-word names, a few common tags and a long tail,
 * one to forty skins with names of their own, and dates over the last few years. `previews`
 * are real strips to borrow, one per pack in turn.
 */
export function madeUpPacks(n: number, previews: string[], seed = 20260923): MockPack[] {
  const next = random(seed);
  const pick = <T,>(list: readonly T[]) => list[next(list.length)];
  const now = 1_790_000_000;
  const named = new Map<string, number>();
  return Array.from({ length: n }, (_, i) => {
    const shape = next(3);
    const base =
      shape === 0 ? `${pick(ADJECTIVES)} ${pick(NOUNS)}` : shape === 1 ? `${pick(THINGS)} ${pick(NOUNS)}` : `${pick(NOUNS)} in ${pick(ADJECTIVES)}`;
    // A name taken already becomes the next in a series, as people number their own.
    const seen = (named.get(base) ?? 0) + 1;
    named.set(base, seen);
    const name = seen === 1 ? base : `${base} ${seen}`;
    const id = name.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase().replace(/[^a-z0-9]+/g, "-");
    // A few tags on many packs, most on few.
    const tags = [...new Set(Array.from({ length: 1 + next(3) }, () => TAGS[Math.min(next(TAGS.length), next(TAGS.length))]))];
    const count = 1 + Math.min(next(40), next(40));
    const skins = Array.from({ length: count }, (_, k) => (next(3) === 0 ? `${pick(ADJECTIVES)} ${pick(THINGS)}` : `${pick(THINGS)} ${k + 1}`));
    return {
      id,
      name,
      author: pick(AUTHORS),
      license: pick(LICENSES),
      tags,
      count,
      bytes: count * (60_000 + next(200_000)),
      hash: (i + 1).toString(16).padStart(16, "0"),
      added: now - next(3 * 365 * 24 * 3600),
      skins,
      preview: previews[i % previews.length],
    };
  });
}

// ---------- the official collection ----------

/** One official skin as the preview's catalog holds it. */
export type MockOfficialSkin = {
  name: string;
  tags: string[];
  sha256: string;
  ext: string;
  bytes: number;
  /** When it joined the collection, in Unix seconds. */
  added: number;
  thumbnail: string;
};

export type MockCollectionQuery = { q: string; sort: CollectionSort; offset: number; limit: number };

/** Official skins a search of the packs brings along, as the app's `Results::collection` does. */
export const COLLECTION_HITS = 24;

/** One official skin's words, and its place in collection.json (from 1), which ties fall back to. */
type IndexedSkin = { skin: MockOfficialSkin; n: number; words: string[]; sortName: string };

/**
 * The official collection: skins that aren't in a pack, searched by name and tag the way the
 * packs are (every word typed the start of one of the skin's), newest first or A to Z.
 */
export class MockCollection {
  private indexed: IndexedSkin[];

  constructor(skins: MockOfficialSkin[]) {
    this.indexed = skins.map((skin, i) => ({ skin, n: i + 1, words: [...words(skin.name), ...skin.tags.flatMap(words)], sortName: words(skin.name).join(" ") }));
  }

  get size(): number {
    return this.indexed.length;
  }

  find(sha256: string): MockOfficialSkin | undefined {
    return this.indexed.find((i) => i.skin.sha256 === sha256)?.skin;
  }

  search({ q, sort, offset, limit }: MockCollectionQuery): { total: number; items: MockOfficialSkin[] } {
    const typed = words(q).slice(0, MAX_WORDS);
    const page = Math.min(Math.max(limit, 1), MAX_PAGE);
    const found = this.indexed.filter((i) => typed.every((w) => i.words.some((x) => x.startsWith(w))));
    // The latest additions lead; a day's additions, the last listed first.
    const ordered =
      sort === "name"
        ? found.sort((a, b) => (a.sortName < b.sortName ? -1 : a.sortName > b.sortName ? 1 : a.n - b.n))
        : found.sort((a, b) => b.skin.added - a.skin.added || b.n - a.n);
    return { total: ordered.length, items: ordered.slice(offset, offset + page).map((i) => i.skin) };
  }

  /** The few official skins a search of the packs brings along: none with nothing typed. */
  hits(q: string): MockOfficialSkin[] {
    if (words(q).length === 0) return [];
    return this.search({ q, sort: "newest", offset: 0, limit: COLLECTION_HITS }).items;
  }
}

/** A made-up SHA-256 that is always the same for the same text: 64 hex digits, from FNV-1a eight ways. */
export function fakeSha256(text: string): string {
  let out = "";
  for (let k = 0; k < 8; k++) {
    let h = 0x811c9dc5 ^ k;
    for (const ch of `${k}:${text}`) h = Math.imul(h ^ ch.charCodeAt(0), 0x01000193);
    out += (h >>> 0).toString(16).padStart(8, "0");
  }
  return out;
}

const SUBJECTS = [
  "Giraffe", "Astronaut", "Lighthouse", "Tiger", "Octopus", "Hummingbird", "Cactus", "Robot", "Whale", "Fox",
  "Panda", "Volcano", "Sunflower", "Jellyfish", "Samurai", "Flamingo", "Rocket", "Teapot", "Bonsai", "Tortoise",
];
const SCENES = ["cola", "at dusk", "in neon", "on paper", "in gold", "in the rain", "dreaming", "at sea", "in bloom", "after dark"];
const OFFICIAL_TAGS = ["pop art", "anime", "retro", "nature", "space", "animals", "minimal", "neon", "vintage", "painting"];

/**
 * `n` made-up official skins, in the order collection.json would list them: a subject and a
 * scene for a name ("Giraffe cola"), up to three tags, and a date over the past year, several on
 * the same day as a batch would be. `pictures` are real ones to borrow, one per skin in turn.
 */
export function madeUpCollection(n: number, pictures: string[], seed = 20260930): MockOfficialSkin[] {
  const next = random(seed);
  const today = 1_790_000_000 - (1_790_000_000 % 86_400);
  const named = new Map<string, number>();
  return Array.from({ length: n }, (_, i) => {
    const base = `${SUBJECTS[next(SUBJECTS.length)]} ${SCENES[next(SCENES.length)]}`;
    const seen = (named.get(base) ?? 0) + 1;
    named.set(base, seen);
    const tags = [...new Set(Array.from({ length: next(4) }, () => OFFICIAL_TAGS[next(OFFICIAL_TAGS.length)]))];
    return {
      name: seen === 1 ? base : `${base} ${seen}`,
      tags,
      sha256: fakeSha256(`official:${i}`),
      ext: "webp",
      bytes: 240_000 + next(1_200_000),
      // Later in collection.json is added later: a batch every few days, going back a year.
      added: today - Math.floor(((n - 1 - i) / 6) * 3) * 86_400,
      thumbnail: pictures[i % pictures.length],
    };
  });
}
