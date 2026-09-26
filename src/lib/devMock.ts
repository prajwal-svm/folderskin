/**
 * Browser-only stand-in for the Tauri commands so `pnpm dev` in a plain browser shows the real
 * layout. The library lives in memory and starts empty, like a first launch; community packs use
 * real pictures from the packs' published tree on GitHub (COMMUNITY_TREE) and the intro's colour
 * folders.
 * Never used inside the app: `isTauri()` is true there.
 *
 * The onboarding shows until it's finished once in this browser; add `?onboarding` to the address
 * to see it again. "Chrome dreams" fails the first time it's added, to show what a failure does,
 * and `?offline` makes everything from packs.folderskin.app and GitHub fail, as it does without a
 * connection. `?real` leaves out the made-up packs, for screenshots.
 *
 * `?update` finds a made-up next version a few seconds after the app opens, as a release build
 * does; `?update=fail` stops its download halfway and `?update=offline` can't check at all.
 *
 * `?yours=8` starts with eight skins of your own, for the parts that need a library to work on.
 *
 * `?bigtree` makes the first folder chosen a made-up Studio with 4,960 folders inside: one of them
 * holds 1,400 and one branch goes 36 levels down, to see choosing folders keep up. `?rushes` makes
 * it a folder of film rushes with 101,040 folders and long names, for runs at that size. The
 * folders "choose a folder" hands out in turn end with a Photo archive of 48,210.
 *
 * `?packs=10000` adds that many made-up packs to Community (mockCommunity.ts), searched the way
 * the app searches its catalog, to see and test the view at the size it is built for. Classic Art
 * and Colours are marked official, as official.json would.
 *
 * `?install=colours` opens the preview the way a folderskin://install link opens the app: on that
 * pack in Community, adding it. The link is taken once, as the app takes one.
 * Sharing works in the preview against a made-up service: `?noshare` shows it as a build without
 * one, `?offline` as one that can't reach it, and `?shared` starts with a few packs already sent,
 * one of them turned down. `?slowdown` has the service turn down the first picture as part of a
 * burst, so the dialog waits and tries again, and `?cooling` has it refuse the pack because the
 * computer is cooling down, which is never tried again.
 */
import { COLOUR_FOLDERS } from "../assets/onboarding";
import { drawOnFolder, loadTemplate, type TemplateImages } from "../composer/composite";
import { fallbackParts, type FolderStyle } from "../composer/parts";
import type {
  AiCatalogue,
  AiGenerateRequest,
  AiModel,
  AiProvider,
  ChatRefDto,
  ChatSummaryDto,
  LocalStatus,
  CommunityPack,
  CommunityQuery,
  CommunitySearch,
  FirstPacks,
  ComposerImage,
  ComposerSaved,
  ComposerSaveHeader,
  ComposerTemplate,
  ExportedPack,
  ExportPackRequest,
  FolderIcon,
  IconPackProgress,
  InstalledIconPack,
  MakeProgress,
  MySubmission,
  PackProgress,
  PackToShare,
  ScaledPicture,
  SharedPack,
  ShareProgress,
  ShareStatus,
  PackSkinPreview,
  PackUpdate,
  PathInfo,
  PlatformInfo,
  RunChoice,
  SavedPrompt,
  Skin,
  SkinList,
} from "./tauri";
import type { ShapeInfo } from "./shapes";
import type { AvailableUpdate } from "./updater";
import type { AiEvent } from "../state/chats";
import type { SubfolderCount, SubfolderCounts, SubfolderList, TreeRun, TreeRunEvent } from "./tree";
import { takes, type Choice } from "./folderChoice";
import { cleanName } from "./names";
import { isImagePath } from "./files";
import { cleanTags } from "./tags";
import { madeUpPacks, MockCatalog, type MockPack as CatalogPack } from "./mockCommunity";
import { styleById } from "./styles";

/** Icon packs "downloaded" in the browser preview, for this page's life. */
const mockIconPacks = new Map<string, string>();

/** Keys "saved" in the browser preview, so the assistant can be walked through end to end. */
const mockKeys = new Set<string>();

/** AI runs stopped in the preview, by job. */
const mockStopped = new Set<string>();

const MOCK_LABELS: Record<string, string> = { openai: "OpenAI", xai: "xAI Grok", recraft: "Recraft", google: "Google Gemini", bfl: "Black Forest Labs", stability: "Stability AI", ideogram: "Ideogram" };

/** Whether the Local Model is set up in the preview (`?localready` starts it set up), and how long
 *  its last picture took: unknown until one is painted, as the app only knows once it has. */
// `?leftovers`: an earlier build's model files (8-bit klein and Z-Image Turbo) still on the disk.
const mockLocal = {
  ready: new URLSearchParams(location.search).has("localready"),
  seconds: null as number | null,
  unused: new URLSearchParams(location.search).has("leftovers") ? 15_158_000_000 : 0,
  /** How much of each of setup's files is on the disk: a stopped setup keeps what came, and the next carries on from there. */
  got: {} as Record<string, number>,
};

/** What setting up downloads in the preview, file by file. */
const MOCK_SETUP_FILES: [string, number][] = [
  ["stable-diffusion.cpp (CUDA)", 150_000_000],
  ["FLUX.2 klein 4B, q4", 2_400_000_000],
  ["Qwen3 4B text encoder, q4", 2_500_000_000],
  ["FLUX.2 autoencoder", 330_000_000],
];
const MOCK_SETUP_BYTES = MOCK_SETUP_FILES.reduce((sum, [, bytes]) => sum + bytes, 0);
const mockGot = () => Object.values(mockLocal.got).reduce((sum, n) => sum + n, 0);

/** The preview's setup under way, which a second aiLocalSetup joins as ai_local_setup does: it
 *  hears where the setup has got to, then what comes next, and settles as the setup does. */
let mockSetup: { listeners: ((event: AiEvent) => void)[]; heard: () => AiEvent[]; done: Promise<LocalStatus> } | null = null;

function mockLocalStatus(): LocalStatus {
  return {
    ready: mockLocal.ready,
    can_set_up: true,
    setting_up: mockSetup !== null,
    backend: "CUDA",
    device: "NVIDIA GeForce RTX 3050 Ti, 4 GB",
    download_bytes: mockLocal.ready ? 0 : MOCK_SETUP_BYTES - mockGot(),
    installs: null,
    kept_bytes: (mockLocal.ready ? MOCK_SETUP_BYTES : mockGot()) + mockLocal.unused,
    unused_bytes: mockLocal.unused,
    model: "FLUX.2 [klein] 4B",
    quality: "4-bit",
    model_bytes: 5_207_178_964,
    // `?lowspace`: a disk with less free than setting up wants.
    free_bytes: new URLSearchParams(location.search).has("lowspace") ? 3_200_000_000 : 180_000_000_000,
    wanted_bytes: mockLocal.ready ? 0 : 8_070_000_000,
    seconds_per_image: mockLocal.seconds,
    home: "C:\\Users\\you\\AppData\\Local\\folderskin-localgen",
    note: null,
  };
}

/** A skin's name as ai.rs `short_name` makes it: the idea's first four words, no more than 28
 *  bytes of them (one long word cut to fit), never ending on a little one, the first letter a capital. */
function mockShortName(idea: string): string {
  const MAX_BYTES = 28;
  const bytes = (s: string) => new TextEncoder().encode(s).length;
  const words: string[] = [];
  let len = 0;
  for (const word of idea.replace(/[,.;:!?]/g, " ").split(/\s+/).filter(Boolean).slice(0, 4)) {
    const add = bytes(word) + (words.length > 0 ? 1 : 0);
    if (len + add > MAX_BYTES) {
      if (words.length === 0) {
        let cut = "";
        for (const ch of word) {
          if (bytes(cut + ch) > MAX_BYTES) break;
          cut += ch;
        }
        words.push(cut);
      }
      break;
    }
    len += add;
    words.push(word);
  }
  while (words.length > 1 && /^(a|an|the|at|of|in|on|with|and|for|to|by)$/i.test(words[words.length - 1])) words.pop();
  const name = words.join(" ");
  return name ? name.charAt(0).toUpperCase() + name.slice(1) : "Generated";
}

/** The preview's saved chats, in this browser's storage so they're there after a reload, as the app's are. */
const CHATS_KEY = "folderskin.mock.chats";
type MockChats = { index: ChatSummaryDto[]; chats: Record<string, unknown> };
function mockChats(): MockChats {
  try {
    const v = JSON.parse(localStorage.getItem(CHATS_KEY) ?? "null") as MockChats | null;
    if (v && Array.isArray(v.index) && v.chats) return v;
  } catch {
    // A fresh start.
  }
  return { index: [], chats: {} };
}
function saveMockChats(store: MockChats) {
  localStorage.setItem(CHATS_KEY, JSON.stringify(store));
}

/** The preview's saved prompts, in this browser's storage like its chats, as the app keeps them. */
const PROMPTS_KEY = "folderskin.mock.prompts";
function mockPrompts(): SavedPrompt[] {
  try {
    const v = JSON.parse(localStorage.getItem(PROMPTS_KEY) ?? "[]") as SavedPrompt[];
    return Array.isArray(v) ? v : [];
  } catch {
    return [];
  }
}
function saveMockPrompts(list: SavedPrompt[]) {
  localStorage.setItem(PROMPTS_KEY, JSON.stringify(list));
}

/** The shapes as ai_shapes lists them: each folder drawn bare on its own template, as the
 *  compositor draws it, and the free icon with no picture. Drawn once. */
let shapesDrawn: Promise<ShapeInfo[]> | null = null;
function mockShapes(): Promise<ShapeInfo[]> {
  shapesDrawn ??= (async () => {
    const bare = async (style: FolderStyle, top: string, bottom: string) => {
      const size = 96;
      const art = document.createElement("canvas");
      art.width = art.height = size;
      const g = art.getContext("2d")!;
      const fill = g.createLinearGradient(0, 0, 0, size);
      fill.addColorStop(0, top);
      fill.addColorStop(1, bottom);
      g.fillStyle = fill;
      g.fillRect(0, 0, size, size);
      const c = document.createElement("canvas");
      c.width = c.height = size;
      drawOnFolder(c.getContext("2d")!, art, await templateImages(style), size, document.createElement("canvas"));
      return c.toDataURL("image/png");
    };
    return [
      { id: "mac-folder", label: "Mac folder", system: "mac", family: "folder", whole: true, thumbnail: await bare("mac", "#7cc8f5", "#4ea9e4") },
      { id: "windows-folder", label: "Windows folder", system: "windows", family: "folder", whole: true, thumbnail: await bare("windows", "#ffe69a", "#ffcc48") },
      { id: "free", label: "Free icon", system: "any", family: "free", whole: false, thumbnail: null },
    ];
  })();
  return shapesDrawn;
}

/** A made-up free icon for the preview: a round character in a colour picked from its idea, on
 *  transparency, as a cut-out free icon comes back. */
function mockIconPicture(idea: string): string {
  const size = 256;
  const c = document.createElement("canvas");
  c.width = c.height = size;
  const g = c.getContext("2d")!;
  const hue = [...idea].reduce((h, ch) => (h * 31 + ch.charCodeAt(0)) % 360, 17);
  const body = g.createRadialGradient(size * 0.4, size * 0.35, size * 0.05, size / 2, size / 2, size * 0.42);
  body.addColorStop(0, `hsl(${hue} 90% 72%)`);
  body.addColorStop(1, `hsl(${hue} 70% 46%)`);
  g.fillStyle = body;
  g.beginPath();
  g.ellipse(size / 2, size * 0.54, size * 0.36, size * 0.34, 0, 0, Math.PI * 2);
  g.fill();
  g.fillStyle = "#1c2230";
  for (const x of [0.4, 0.6]) {
    g.beginPath();
    g.arc(size * x, size * 0.48, size * 0.035, 0, Math.PI * 2);
    g.fill();
  }
  g.strokeStyle = "#1c2230";
  g.lineWidth = size * 0.02;
  g.lineCap = "round";
  g.beginPath();
  g.arc(size / 2, size * 0.56, size * 0.07, 0.15 * Math.PI, 0.85 * Math.PI);
  g.stroke();
  return c.toDataURL("image/png");
}

/** A pack as the preview lists it, before whether it's added (or changed) is worked out. Made-up
 *  packs also carry their skins' names. */
type MockPack = Omit<CommunityPack, "added" | "update" | "hash" | "bytes" | "preview" | "official"> & { skins?: string[] };

/** The packs the preview marks official, as official.json does in folderskin-community. */
const MOCK_OFFICIAL = new Set(["classic-art", "colours"]);
/** Whether `?install=` has been taken already: a link is taken once. */
let mockLinkTaken = false;

/** Sample packs for the browser preview's Community view. The real list comes from packs.folderskin.app. */
/** The packs' published tree, where the preview's real pictures come from. Every file there is named after its
 *  contents (`folderskin_catalog::tree`), so these addresses stay the same when a pack is given a new id. Declared
 *  before anything that runs at load: `library` is seeded from picture(), which reads it. */
const COMMUNITY_TREE = "https://raw.githubusercontent.com/prajwal-svm/folderskin-community/main/v2";
const MOCK_PACKS: MockPack[] = [
  { id: "classic-art", name: "Classic Art", author: "prajwal-svm", license: "CC0-1.0", tags: ["classic art"], count: 16 },
  { id: "colours", name: "Colours", author: "prajwal-svm", license: "CC0-1.0", tags: ["colour"], count: 8 },
  { id: "night-prints", name: "Night prints", author: "example", license: "CC-BY-4.0", tags: ["woodblock", "night", "animals"], count: 12 },
  { id: "chrome-dreams", name: "Chrome dreams", author: "example", license: "CC-BY-4.0", tags: ["airbrush", "retro"], count: 6 },
];
/** Real packs' preview strips in the published tree, by the version (the pack's hash) each shows. */
const STRIPS = {
  "classic-art": `${COMMUNITY_TREE}/strips/ecfcc6015c286fba.webp`,
  colours: `${COMMUNITY_TREE}/strips/f0d421a416f6e2c9.webp`,
  "greek-art": `${COMMUNITY_TREE}/strips/e04c64b06fbf385a.webp`,
  "scientists-pop-art": `${COMMUNITY_TREE}/strips/0c577ff4cd8f80e6.webp`,
  "soft-rainbow": `${COMMUNITY_TREE}/strips/6f600f4adcaa7361.webp`,
};
/** The strip each sample pack shows; the made-up packs borrow a real one. */
const PREVIEW_OF: Record<string, string> = {
  "classic-art": STRIPS["classic-art"],
  colours: STRIPS.colours,
  "night-prints": STRIPS["classic-art"],
  "chrome-dreams": STRIPS.colours,
};
/** Classic Art's pictures, finished folders already, in the pack's order: each picture's SHA-256, which the
 *  published tree names it after, and its name. When the pack's pictures change, the old ones leave the tree,
 *  so take the new SHA-256s (and the pack's hash, for STRIPS) from its manifest there, v2/packs/<id>/<hash>.json. */
const CLASSIC_ART = [
  ["4357c4c3c68d4dca8cf1a1aec58a35d85bbf9322b52211e1700b8bc534fe835f", "Mona Lisa"],
  ["ebf2414e6b02a8f49471ca7399c5c8b62449c6673d0025c6842d03ad9a44644f", "View of Toledo"],
  ["25fd5db6138b79269dce6192e8bb3cbd404a0b851641fbf3b4011258105e9bdc", "Girl with a Pearl Earring"],
  ["c54b8535c063a0612c6dad248728c5bbb7fb372f3d76acb89a4777f95696e20f", "The Astronomer"],
  ["526ab5a526828e63a0fe358467bf768b5f23fe54219c29297b8e1b3cc41dc7e6", "Oath of the Horatii"],
  ["4571a171c82726a47c7babd152ca83f60e66d8a8964fb039d0d07771671fb722", "Napoleon Crossing the Alps"],
  ["533ba527c7f69745c461b388c7eb88ba07adf035fda6ef01bcfbd3daced5f085", "Wanderer above the Sea of Fog"],
  ["d12b2b381d40788fc8c13f48418d2e0a2eb846c292a16261d71f760fe8bdaf36", "The Ninth Wave"],
  ["0e0f4ce33e12291d4f80d6e93f8525434ab7f99949dddca8210c7f17074b25d1", "Boulevard des Capucines"],
  ["870385b3a9e1634ddbe8ad735c3d39053857774d82b52ce59bb8e8266a25d2a2", "Breezing Up"],
  ["ff2895b12dd7f08812e234bc105ee46e2722cf700f73e4f8f483f800429daebc", "Paris Street; Rainy Day"],
  ["e62e43b6abe4af04ac70db2bec33155e4f35a2e9b30fd3e68ec0c0539cdbeaa5", "Luncheon of the Boating Party"],
  ["b5000578e8e8b4c667be71de93a8fc913e3cccd45733febba9140cb018dcfd0d", "The Lady of Shalott"],
  ["4280a6043347491d24019dcaf9497a12d37b55c0b348268d8342b68f1df53044", "The Starry Night"],
  ["14b5a83486c65b6189cb709234294656cf3965bb2e6e821c34237282fc15cb40", "Mont Sainte-Victoire"],
  ["b4704a8e30578b1558f61c00e306cb8dd0fb87517800b8a575ba184a535a40f8", "Composition VIII"],
] as const;
/** Where a picture of Classic Art's is, by its SHA-256. */
const classicArt = (sha256: string) => `${COMMUNITY_TREE}/pictures/${sha256}.webp`;
const COLOUR_NAMES = ["Blue", "Orange", "Purple", "Green"];

/** `?yours=8` starts with that many skins of your own, so sharing can be tried without making any. */
function seeded(): Skin[] {
  const many = Number(new URLSearchParams(location.search).get("yours") ?? "0");
  if (!Number.isFinite(many) || many < 1) return [];
  const now = Date.now();
  return Array.from({ length: Math.min(many, 40) }, (_, i) => ({
    id: `user:seed${i}`,
    name: CLASSIC_ART[i % CLASSIC_ART.length][1],
    collection: "yours",
    thumbnail: picture(i),
    custom: true,
    kind: "artwork" as const,
    source: "import" as const,
    created_at: now - i,
    tags: i % 3 === 0 ? ["painting"] : ["photo"],
  }));
}

/** Everything in the preview's library, newest first. */
let library: Skin[] = seeded();
/** Sample packs added before their "new version": Colours gets one the first time it is added. */
const mockStale = new Set<string>();
/** Packs that have already failed once, so the next try works. */
const mockFailedOnce = new Set<string>();
/** Packs looked through already, which the app would show from its cache. */
const mockViewed = new Set<string>();

const ONBOARDED_KEY = "folderskin.mock.onboarded";
const OFFLINE = "couldn't reach packs.folderskin.app. Check your connection and try again";
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const offline = () => new URLSearchParams(location.search).has("offline");
/** The packs that really are on GitHub; the others only show what a longer list looks like. */
const REAL_PACKS = new Set(["classic-art", "colours"]);
const listed = () => (new URLSearchParams(location.search).has("real") ? MOCK_PACKS.filter((p) => REAL_PACKS.has(p.id)) : MOCK_PACKS);

/** A stand-in skin picture: one of Classic Art's, or one of the intro's colour folders. */
function picture(i: number): string {
  return i % 3 === 2 ? COLOUR_FOLDERS[i % COLOUR_FOLDERS.length] : classicArt(CLASSIC_ART[i % CLASSIC_ART.length][0]);
}

/** What a pack's skins look like here: the real pictures for Classic Art and Colours. */
function packPictures(pack: MockPack): { name: string; thumbnail: string }[] {
  return Array.from({ length: pack.count }, (_, i) => {
    if (pack.id === "classic-art") return { name: CLASSIC_ART[i][1], thumbnail: classicArt(CLASSIC_ART[i][0]) };
    if (pack.id === "colours") return { name: COLOUR_NAMES[i % 4] + (i >= 4 ? " 2" : ""), thumbnail: COLOUR_FOLDERS[i % 4] };
    return { name: pack.skins?.[i] ?? `${pack.name} ${i + 1}`, thumbnail: picture(i + 5) };
  });
}

/** The real preview strips, which the made-up packs borrow in turn. */
const REAL_PREVIEWS = Object.values(STRIPS);
/** When the sample packs were published, newest first. */
const SAMPLE_DATES: Record<string, number> = { "classic-art": 1_780_000_000, colours: 1_770_000_000, "night-prints": 1_760_000_000, "chrome-dreams": 1_750_000_000 };

let mockCatalogue: MockCatalog | null = null;
/** Whether the preview's catalog has been "downloaded" yet: the first search waits for it, as in the app. */
let mockCatalogueLoaded = false;

/** The preview's catalog: the sample packs, then `?packs=N` made-up ones, made the first time it is asked for. */
function communityCatalog(): MockCatalog {
  if (mockCatalogue) return mockCatalogue;
  const many = Math.min(Math.max(Number(new URLSearchParams(location.search).get("packs") ?? "0") || 0, 0), 50_000);
  const samples: CatalogPack[] = listed().map((p, i) => ({
    ...p,
    // A version of their own, so the library can say which one it has.
    hash: (0xf00 + i).toString(16).padStart(16, "0"),
    bytes: p.count * 180_000,
    added: SAMPLE_DATES[p.id] ?? 0,
    skins: packPictures(p).map((s) => s.name),
    preview: PREVIEW_OF[p.id] ?? STRIPS.colours,
  }));
  mockCatalogue = new MockCatalog([...samples, ...madeUpPacks(many, REAL_PREVIEWS)], ["classic-art", "colours"]);
  return mockCatalogue;
}

/** A catalog pack as the webview gets it, marked against the library. */
function communityPack(p: CatalogPack): CommunityPack {
  const added = packAdded(p.id);
  return {
    id: p.id,
    name: p.name,
    author: p.author,
    license: p.license,
    tags: p.tags,
    count: p.count,
    bytes: p.bytes,
    hash: p.hash,
    preview: p.preview,
    added,
    update: added && mockStale.has(p.id),
    official: MOCK_OFFICIAL.has(p.id),
  };
}

/** Any pack the preview knows, sample or made up. */
const findPack = (id: string): MockPack | undefined => MOCK_PACKS.find((p) => p.id === id) ?? communityCatalog().find(id);

function mockPackSkins(pack: MockPack): Skin[] {
  const now = Date.now();
  return packPictures(pack).map((p, i) => ({
    id: `user:${pack.id}${i}`,
    name: p.name,
    collection: "yours",
    thumbnail: p.thumbnail,
    custom: true,
    kind: "folder" as const,
    source: "community" as const,
    // The pack's first skin is the newest, as the app does it, so the library shows the pack in order.
    created_at: now + pack.count - i,
    tags: pack.tags,
    pack: pack.id,
    pack_name: pack.name,
    author: pack.author,
    license: pack.license,
  }));
}

/** Puts skins in the library, replacing any with the same id. */
function keep(skins: Skin[]) {
  library = [...skins, ...library.filter((s) => !skins.some((k) => k.id === s.id))];
}

const packAdded = (id: string) => library.some((s) => s.pack === id);

/** A folder of thousands with `?bigtree`: see `bigTree`. */
const BIG_TREE = "/Users/you/Documents/Studio";
/** Folders the preview's "choose a folder" hands out in turn, so switching folders can be tried.
 *  With `?bigtree`, the big one comes first. */
/** A tree of over a hundred thousand folders with long names, with `?rushes`: see `rushes`. */
const RUSHES = "/Users/you/Movies/Rushes from the lighthouse documentary, every camera and every day of the shoot";
const SAMPLE_FOLDERS = [
  ...(new URLSearchParams(location.search).has("bigtree") ? [BIG_TREE] : []),
  ...(new URLSearchParams(location.search).has("rushes") ? [RUSHES] : []),
  "/Users/you/Documents/Projects",
  "/Users/you/Pictures/Wedding",
  "/Users/you/Desktop/Taxes 2026",
  "/Users/you/Pictures/Photo archive",
];
/** A tree as big as a network share: 48,210 folders, counted over a moment or two (`photoArchive`). */
const HUGE_TREE = "/Users/you/Pictures/Photo archive";
let nextSample = 0;
/**
 * The icon each folder wears in the preview: Projects, the rushes (and the big tree) start plain
 * and the others with a colour of their own, so a custom icon can be tried. Applying and
 * reverting change it.
 */
const mockIcons = new Map<string, string | null>(
  SAMPLE_FOLDERS.map((path, i) => [path, path === BIG_TREE || path === RUSHES || path.endsWith("/Projects") ? null : COLOUR_FOLDERS[i % COLOUR_FOLDERS.length]]),
);

/**
 * The folders inside each sample folder, for trying "Include subfolders" and choosing some of them:
 * Projects has plenty, one branch six levels deep, Wedding has one the preview can't change (to show
 * a partial result), Taxes 2026 has none, and Photo archive has 48,210 (`photoArchive`).
 */
const SAMPLE_TREES: Record<string, string[]> = {
  "/Users/you/Documents/Projects": [
    "Clients", "Design", "Invoices", "Notes", "Photos", "Research", "Templates", "Videos",
    "Clients/Acme", "Clients/Globex", "Clients/Initech", "Design/Icons", "Design/Mockups",
    "Invoices/2025", "Invoices/2026", "Photos/2019", "Photos/2020", "Photos/2021", "Research/Papers",
    "Templates/Letters", "Videos/Edited", "Videos/Raw", "Clients/Acme/Contracts", "Photos/2021/Holiday",
    "Research/Papers/2026", "Research/Papers/2026/Drafts", "Research/Papers/2026/Drafts/Figures",
    "Research/Papers/2026/Drafts/Figures/Final",
  ].map((p) => `/Users/you/Documents/Projects/${p}`),
  "/Users/you/Pictures/Wedding": ["Ceremony", "Guests", "Private", "Reception", "Ceremony/Rings", "Reception/Speeches, toasts and the first dance"].map(
    (p) => `/Users/you/Pictures/Wedding/${p}`,
  ),
};

/**
 * `?bigtree`'s Studio: 4,960 folders, to see the chooser keep up. Camera roll holds 1,400 folders
 * side by side, Deep dive goes 36 levels down, and Clients, Music, Research and Scans are wide and
 * bushy around them.
 */
function bigTree(): string[] {
  const paths: string[] = ["Camera roll", "Clients", "Deep dive", "Music", "Research", "Scans"];
  for (let day = 1; day <= 1400; day++) paths.push(`Camera roll/Day ${day}`);
  for (let c = 1; c <= 60; c++) {
    paths.push(`Clients/Client ${c}`);
    for (let p = 1; p <= 3; p++) {
      paths.push(`Clients/Client ${c}/Project ${p}`);
      for (const part of ["Brief", "Drafts", "Final", "Invoices"]) paths.push(`Clients/Client ${c}/Project ${p}/${part}`);
    }
  }
  let deep = "Deep dive";
  for (let level = 1; level <= 36; level++) paths.push((deep = `${deep}/Level ${level}`));
  for (let a = 1; a <= 200; a++) {
    paths.push(`Music/Artist ${a}`);
    for (let album = 1; album <= 4; album++) paths.push(`Music/Artist ${a}/Album ${album}`);
  }
  const topics = ["Topic A", "Topic B", "Topic C", "Topic D"];
  for (const a of topics) {
    paths.push(`Research/${a}`);
    for (let b = 1; b <= 4; b++) {
      paths.push(`Research/${a}/Part ${b}`);
      for (let c = 1; c <= 4; c++) {
        paths.push(`Research/${a}/Part ${b}/Section ${c}`);
        for (let d = 1; d <= 4; d++) paths.push(`Research/${a}/Part ${b}/Section ${c}/Note ${d}`);
      }
    }
  }
  for (let scan = 1; paths.length < 4960; scan++) paths.push(`Scans/Scan ${scan}`);
  // Nearest first, as the walk finds them; each level keeps the order it was made in.
  const depth = (path: string) => path.split("/").length;
  return paths.sort((a, b) => depth(a) - depth(b)).map((p) => `${BIG_TREE}/${p}`);
}

const MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/**
 * Photo archive's folders, made up as they're asked for: thirty years, twelve months in each, and
 * 132 or 133 days in each month, 48,210 folders in all.
 */
function photoArchive(path: string): string[] {
  const rest = path.slice(HUGE_TREE.length).split("/").filter(Boolean);
  if (rest.length === 0) return Array.from({ length: 30 }, (_, i) => String(1997 + i));
  if (rest.length === 1) return MONTHS;
  if (rest.length === 2) {
    const month = (Number(rest[0]) - 1997) * 12 + MONTHS.indexOf(rest[1]);
    return Array.from({ length: month < 300 ? 133 : 132 }, (_, i) => `Day ${i + 1}`);
  }
  return [];
}

const PLACES = ["the north headland", "the lamp room", "the keeper's cottage", "the harbour wall", "the supply boat", "the lighthouse gallery", "the old fog signal", "the village hall"];

/**
 * The rushes' folders, made up as they're asked for: forty days of the shoot, twenty-five camera
 * rolls a day and a hundred clips a roll, 101,040 folders in all, most of them with long names.
 * Day 17's are locked the first time a run gets to them, so a big run has failures to try again.
 */
function rushes(path: string): string[] {
  const rest = path.slice(RUSHES.length).split("/").filter(Boolean);
  const two = (n: number) => String(n).padStart(2, "0");
  if (rest.length === 0) return Array.from({ length: 40 }, (_, i) => `Day ${two(i + 1)} at ${PLACES[i % PLACES.length]}, with the keeper's family and the whole crew`);
  if (rest.length === 1) return [..."ABCDE"].flatMap((camera) => Array.from({ length: 5 }, (_, r) => `Camera ${camera} roll ${two(r + 1)}, interviews and cutaways before the storm came in`));
  if (rest.length === 2) return Array.from({ length: 100 }, (_, i) => `Clip ${String(i + 1).padStart(4, "0")}`);
  return [];
}
/** The rushes' folders that have failed once, and work the next time. */
const lockedOnce = new Set<string>();

/** Each explicit sample tree's folders by the folder they're in, made the first time it's asked for. */
const insideOf = new Map<string, Map<string, string[]>>();

/** The names of the folders directly inside `path`, in any of the preview's sample folders. */
function mockChildren(path: string): string[] {
  if (path === HUGE_TREE || path.startsWith(`${HUGE_TREE}/`)) return photoArchive(path);
  if (path === RUSHES || path.startsWith(`${RUSHES}/`)) return rushes(path);
  const root = SAMPLE_FOLDERS.find((r) => path === r || path.startsWith(`${r}/`));
  if (!root) return [];
  if (root === BIG_TREE) SAMPLE_TREES[BIG_TREE] ??= bigTree();
  let byParent = insideOf.get(root);
  if (!byParent) {
    byParent = new Map();
    for (const p of SAMPLE_TREES[root] ?? []) {
      const cut = p.lastIndexOf("/");
      const list = byParent.get(p.slice(0, cut)) ?? [];
      list.push(p.slice(cut + 1));
      byParent.set(p.slice(0, cut), list);
    }
    insideOf.set(root, byParent);
  }
  return byParent.get(path) ?? [];
}

/** Folders in the sample trees that wear an icon of their own. */
const mockTreeIcons = new Set<string>(["/Users/you/Pictures/Wedding/Guests"]);
const lastPart = (path: string) => path.split("/").pop() || path;
const ancestorsOf = (root: string, path: string) => {
  const out: string[] = [];
  for (let at = path; at.length > root.length; at = at.slice(0, at.lastIndexOf("/"))) out.push(at.slice(0, at.lastIndexOf("/")));
  return out;
};

/** Everyone listening for runs and counts, as `listen` has them in the app. */
const runListeners = new Set<(event: TreeRunEvent) => void>();
const countListeners = new Set<(count: SubfolderCount) => void>();
let runSeq = 0;

/**
 * The count of the folder on show, depth first as the app counts: each folder's insides found,
 * and how many in its tree are still to read. `?holdcount` holds a big count at 12,400 folders
 * until the page calls `mockCountGo()`, so a test can see one going.
 */
type Counting = { folder: string; found: number; done: boolean; inside: Map<string, number>; open: Map<string, number>; read: Set<string>; stopped: boolean };
let counting: Counting | null = null;

function countNow(c: Counting): SubfolderCount {
  return { folder: c.folder, count: c.found, done: c.done };
}

async function runCount(c: Counting) {
  const stack = [c.folder];
  c.open.set(c.folder, 1);
  let since = 0;
  let holding = new URLSearchParams(location.search).has("holdcount");
  while (stack.length > 0) {
    if (c.stopped) return;
    const folder = stack.pop()!;
    const kids = mockChildren(folder);
    c.read.add(folder);
    for (let i = kids.length - 1; i >= 0; i--) {
      const kid = `${folder}/${kids[i]}`;
      stack.push(kid);
      c.inside.set(kid, 0);
      c.open.set(kid, 1);
    }
    for (const at of [folder, ...ancestorsOf(c.folder, folder)]) {
      c.inside.set(at, (c.inside.get(at) ?? 0) + kids.length);
      c.open.set(at, (c.open.get(at) ?? 0) + kids.length - 1);
    }
    c.found += kids.length;
    since += 1;
    // A thousand or so folders at a time, and a word to whoever's listening after each.
    if (since >= 700) {
      since = 0;
      for (const listener of countListeners) listener(countNow(c));
      if (holding && c.found >= 12_400) {
        holding = false;
        try {
          await held("holdcount", "mockCountGo", () => void (c.stopped && stop()));
        } catch {
          return;
        }
      }
      await sleep(30);
    }
  }
  c.done = true;
  for (const listener of countListeners) listener(countNow(c));
}

/** Throws to give up waiting. */
function stop(): never {
  throw new Error("stopped");
}

/**
 * The preview's run over a tree: the same record the app keeps (src-tauri/src/tree/job.rs), the
 * folders it takes found a stretch ahead of the ones it has done, so "3,120 of 12,000+" shows
 * until they're all found. A small run takes a moment a folder, and a big one a few seconds in
 * all. `?holdrun` holds the page's first run after its first few folders (3,120 of a big one)
 * until the page calls `mockRunGo()`, and `?unfocused` has the window behind others when it ends.
 */
type MockJob = {
  run: TreeRun;
  /** Every folder the run takes, nearest first, as the walk would find them. */
  plan: string[];
  /** Each folder's part: "to do", "changed", "skipped", "failed". */
  parts: Map<string, "to do" | "changed" | "skipped" | "failed">;
  reasons: Map<string, string>;
  act: (path: string) => "changed" | "skipped" | string;
  thumb: string | null;
  going: boolean;
  stop: boolean;
  held: boolean;
};
let job: MockJob | null = null;
let jobIds = 0;

function tellRun() {
  const event: TreeRunEvent = { seq: ++runSeq, run: job ? { ...job.run, failures: [...job.run.failures] } : null };
  for (const listener of runListeners) listener(event);
  return event;
}

/** Which folders a run takes, by the rules `choice` gives: the same as `takes` in lib/folderChoice.ts. */
function planOf(root: string, choice: RunChoice | null): string[] {
  const rules: Choice = { root, separator: "/", all: choice?.all ?? true, rules: Object.fromEntries((choice?.rules ?? []).map((r) => [r.path, r.on])) };
  const ticked = Object.entries(rules.rules).filter(([, on]) => on).map(([p]) => p);
  const plan = [root];
  let level = [root];
  while (level.length > 0) {
    const next: string[] = [];
    for (const folder of level) {
      for (const name of mockChildren(folder)) {
        const path = `${folder}/${name}`;
        const on = takes(rules, path);
        if (on) plan.push(path);
        if (on || ticked.some((p) => p.startsWith(`${path}/`))) next.push(path);
      }
    }
    level = next;
  }
  return plan;
}

function newJob(kind: "apply" | "revert", folder: string, plan: string[], act: MockJob["act"], extra: Partial<TreeRun>, thumb: string | null): MockJob {
  return {
    run: {
      id: ++jobIds,
      kind,
      undoing: false,
      leaves_plain: false,
      folder,
      root: folder,
      name: lastPart(folder),
      skin_id: null,
      running: true,
      stopping: false,
      stopped: false,
      done: 0,
      total: 1,
      counted: plan.length === 1,
      current: lastPart(folder),
      changed: 0,
      failed: 0,
      skipped: 0,
      failures: [],
      error: null,
      ...extra,
    },
    plan,
    parts: new Map(plan.map((p) => [p, "to do"])),
    reasons: new Map(),
    act,
    thumb,
    going: false,
    stop: false,
    held: false,
  };
}

/** Works through the job's folders still to do, found a stretch ahead, until they're done or it's stopped. */
async function work(j: MockJob) {
  j.going = true;
  j.stop = false;
  const r = j.run;
  r.running = true;
  r.stopping = false;
  r.stopped = false;
  tellRun();
  const big = j.plan.length > 200;
  // A small run a moment a folder, a big one in a few seconds.
  const each = big ? Math.ceil(j.plan.length / 150) : 1;
  const pause = big ? 40 : Math.max(1, Math.min(140, 7000 / j.plan.length));
  const hold = new URLSearchParams(location.search).has("holdrun") && j.run.id === 1 ? (big ? 3120 : 3) : -1;
  let i = 0;
  while (true) {
    if (j.stop) break;
    await sleep(pause);
    if (j.stop) break;
    // The walk keeps well ahead of the work: a small tree is all found at once, and a big one a
    // stretch at a time.
    r.total = big ? Math.max(r.total, Math.min(j.plan.length, (r.done + each) * 4 + 1)) : j.plan.length;
    r.counted = r.total === j.plan.length;
    let did = 0;
    for (; i < j.plan.length && did < each; i++) {
      const path = j.plan[i];
      if (j.parts.get(path) !== "to do") continue;
      if (i >= r.total) break;
      const outcome = j.act(path);
      did += 1;
      r.done += 1;
      r.current = lastPart(path);
      if (outcome === "changed" || outcome === "skipped") {
        j.parts.set(path, outcome);
        if (outcome === "changed") r.changed += 1;
        else r.skipped += 1;
      } else {
        j.parts.set(path, "failed");
        j.reasons.set(path, outcome);
        r.failed += 1;
        if (r.failures.length < 100) r.failures.push({ path, name: lastPart(path), reason: outcome });
      }
      if (r.done === hold && !j.held) break;
    }
    tellRun();
    if (r.done === hold && !j.held) {
      j.held = true;
      try {
        await held("holdrun", "mockRunGo", () => void (j.stop && stop()));
      } catch {
        break;
      }
    }
    if (i >= j.plan.length) break;
  }
  const left = j.plan.some((p) => j.parts.get(p) === "to do");
  r.running = false;
  r.stopping = false;
  r.stopped = j.stop && left;
  r.total = left && !r.counted ? r.total : j.plan.length;
  r.counted = !left || r.counted;
  j.going = false;
  if (r.kind === "apply" && j.parts.get(r.root) === "changed") mockIcons.set(r.root, j.thumb);
  if (r.kind === "revert" && j.parts.get(r.root) === "changed") mockIcons.set(r.root, null);
  tellRun();
}

function mockStart(kind: "apply" | "revert", folder: string, plan: string[], act: MockJob["act"], extra: Partial<TreeRun>, thumb: string | null): TreeRunEvent {
  if (job?.run.running) throw new Error(`wait for the run in ${job.run.name} to finish, or stop it, before starting another`);
  job = newJob(kind, folder, plan, act, extra, thumb);
  void work(job);
  return tellRun();
}

/** The latest job, as run `id`, or why it isn't there. */
function jobOf(id: number): MockJob {
  if (!job || job.run.id !== id) throw new Error("that run isn't there any more");
  return job;
}

/** The next sample folder, for the preview's "choose a folder". */
export function mockPickFolder(): string {
  const path = SAMPLE_FOLDERS[nextSample % SAMPLE_FOLDERS.length];
  nextSample += 1;
  return path;
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Notes for the made-up update, written the way a CHANGELOG.md section is. */
const MOCK_NOTES = `### Added

- FolderSkin updates itself: it looks for a new version when it opens, shows what changed, and
  restarts into it.
- A **Scientists - Pop Art** community pack.

### Fixed

- Cut-outs on a plain grey or black background keep dark clothes and ink lines.

---

**Downloads:** macOS \`.dmg\`, Windows \`-setup.exe\` or \`.msi\`, Linux \`.AppImage\`, \`.deb\` or \`.rpm\`.`;

/** The browser preview's update check: see `?update` at the top. */
export async function mockFindUpdate(): Promise<AvailableUpdate | null> {
  const mode = new URLSearchParams(location.search).get("update");
  await sleep(900);
  if (mode === null) return null;
  if (mode === "offline" || offline()) throw new Error("Could not fetch a valid release JSON from the remote");
  return {
    version: "0.2.0",
    notes: MOCK_NOTES,
    install: async (onProgress) => {
      onProgress(0);
      for (let step = 1; step <= 24; step++) {
        await sleep(110);
        if (mode === "fail" && step === 13) throw new Error("error decoding response body");
        onProgress(step / 24);
      }
      await sleep(500);
    },
  };
}

/**
 * The composer's folder layers in the browser preview: the same pictures the Rust side makes, as
 * `folderskin-tools composer-layers` wrote them into docs/images/composer (served in dev only).
 */
const mockTemplateUrls = (style: FolderStyle) => {
  const dir = style === "windows" ? "/docs/images/composer/windows" : "/docs/images/composer";
  return { back: `${dir}/back.png`, front: `${dir}/front.png`, middle: `${dir}/middle.png`, top: `${dir}/top.png`, outline: `${dir}/outline.png` };
};
const mockTemplates = new Map<FolderStyle, Promise<TemplateImages>>();
const templateImages = (style: FolderStyle) => {
  if (!mockTemplates.has(style)) mockTemplates.set(style, loadTemplate(mockTemplateUrls(style)));
  return mockTemplates.get(style)!;
};

/** Which folder the preview puts skins on, kept like the app keeps it. */
const MOCK_LOOK_KEY = "folderskin.mock.look";

/** Designs "saved" in the browser preview, by skin id, so Edit design can be tried. */
const mockDesigns = new Map<string, unknown>();

function base64(bytes: Uint8Array): string {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

function loadImg(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("couldn't read that picture"));
    img.src = src;
  });
}

/** What the Rust side would render for a design: on the folder, or as it is for a free icon. */
async function mockIcon(png: Uint8Array, shape: "folder" | "free", style: FolderStyle, size: number): Promise<string> {
  const img = await loadImg(`data:image/png;base64,${base64(png)}`);
  const c = document.createElement("canvas");
  c.width = size;
  c.height = size;
  const ctx = c.getContext("2d")!;
  ctx.imageSmoothingQuality = "high";
  if (shape === "folder") drawOnFolder(ctx, img, await templateImages(style), size, document.createElement("canvas"));
  else ctx.drawImage(img, 0, 0, size, size);
  return c.toDataURL("image/png");
}

/** A made-up photo for "Choose a picture" in the browser: a lake at sunset, no file needed. */
function mockPhoto(): ComposerImage {
  const c = document.createElement("canvas");
  c.width = 1600;
  c.height = 1000;
  const g = c.getContext("2d")!;
  const sky = g.createLinearGradient(0, 0, 0, 640);
  sky.addColorStop(0, "#2b1f5c");
  sky.addColorStop(0.55, "#e0628a");
  sky.addColorStop(1, "#ffbf6b");
  g.fillStyle = sky;
  g.fillRect(0, 0, 1600, 640);
  g.fillStyle = "#fff1c9";
  g.beginPath();
  g.arc(1040, 560, 120, 0, Math.PI * 2);
  g.fill();
  g.fillStyle = "#3b2a57";
  g.beginPath();
  g.moveTo(0, 640);
  g.bezierCurveTo(300, 420, 560, 560, 820, 500);
  g.bezierCurveTo(1100, 440, 1320, 560, 1600, 470);
  g.lineTo(1600, 1000);
  g.lineTo(0, 1000);
  g.fill();
  const lake = g.createLinearGradient(0, 640, 0, 1000);
  lake.addColorStop(0, "#f39a7d");
  lake.addColorStop(1, "#40285f");
  g.fillStyle = lake;
  g.fillRect(0, 700, 1600, 300);
  return { url: c.toDataURL("image/jpeg", 0.9), width: 1600, height: 1000, name: "Lake at sunset", alpha: false };
}

/** The made-up community service: this computer's key and handle, and the packs it has sent. */
const mockShare: { key: boolean; handle: string | null; wanted: string; waiting: boolean; submissions: MySubmission[] } = {
  key: false,
  handle: null,
  wanted: "",
  waiting: false,
  submissions: [],
};
const SHARE_NOT_YET = "This build of FolderSkin can't share packs. You can still save the pack as a folder.";
const SHARE_UNREACHABLE = "FolderSkin's sharing service can't be reached right now. Check your connection and try again.";
/** The version of the pack terms the made-up service sends packs under. It refuses a pack
 *  agreed under any other, as the service does, so the tests see the app send the service's. */
const MOCK_TERMS_VERSION = 2;
/** What the service says to a computer cooling down after too many refused requests (`?cooling`). */
const SHARE_COOLING = "You've sent too many requests that were turned down. You can share again in 2 hours.";

/** Making a pack's pictures ready, as the app does several at once: `total` of them, a little
 *  while each, told as each is done. */
async function mockEncode(total: number, onProgress: (p: MakeProgress) => void) {
  onProgress({ done: 0, total });
  for (let done = 1; done <= total; done++) {
    await sleep(120);
    onProgress({ done, total });
  }
}

/** The pictures made smaller to fit: none, or with `?scaled` the first one, as a picture too
 *  detailed for 1.5 MB at 1024 px is. */
function mockScaled(skinIds: string[]): ScaledPicture[] {
  if (!new URLSearchParams(location.search).has("scaled") || skinIds.length === 0) return [];
  const first = library.find((s) => s.id === skinIds[0]);
  return [{ name: first?.name ?? "A skin", side: 896 }];
}

/** A new id for a pack called `name`, the way `pack::new_id` draws one: the name as a slug, then
 *  six random characters. */
function mockNewId(name: string): string {
  const base =
    name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 33)
      .replace(/-+$/, "") || "pack";
  const alphabet = "abcdefghijklmnopqrstuvwxyz234567";
  const suffix = Array.from({ length: 6 }, () => alphabet[Math.floor(Math.random() * alphabet.length)]).join("");
  return `${base}-${suffix}`;
}

/** `?shared` starts as a computer that has shared before: verified, one pack approved and one turned down. */
function seedShared() {
  if (!new URLSearchParams(location.search).has("shared") || mockShare.key) return;
  const day = 86400;
  const now = Math.floor(Date.now() / 1000);
  mockShare.key = true;
  mockShare.handle = "sunny-otter";
  mockShare.submissions = [
    {
      id: "sub_mockturneddown00000",
      name: "Neon cats",
      status: "rejected",
      pictures: 6,
      license: "CC0-1.0",
      created_at: now - 3 * day,
      decided_at: now - 2 * day,
      pack_id: null,
      pulled: false,
      reasons: [{ code: "brand", term: 13, message: "The pictures use someone else's logo, trade mark or characters." }],
      note: "The cat in the third picture is a cartoon character that belongs to a studio.",
    },
    {
      id: "sub_mockapproved0000000",
      name: "Night prints",
      status: "approved",
      pictures: 12,
      license: "CC-BY-4.0",
      created_at: now - 9 * day,
      decided_at: now - 7 * day,
      pack_id: "night-prints",
      pulled: true,
      reasons: [],
      note: "",
    },
  ];
}

function mockShareStatus(): ShareStatus {
  seedShared();
  const params = new URLSearchParams(location.search);
  const unavailable = (reason: string): ShareStatus => ({ available: false, reason, verified: false, handle: null, has_key: mockShare.key, terms_version: null });
  if (params.has("noshare")) return unavailable(SHARE_NOT_YET);
  if (offline()) return unavailable(SHARE_UNREACHABLE);
  return {
    available: true,
    reason: null,
    verified: mockShare.handle !== null,
    handle: mockShare.handle,
    has_key: mockShare.key,
    terms_version: MOCK_TERMS_VERSION,
  };
}

/** Set when the folder look changes, so the next list of skins takes as long as a redraw would. */
let mockRedraw = false;

/**
 * For the `?hold…` switches, which keep something under way for a test to look at, however busy
 * the machine: whether `switch` is on, and if so, waits until the page calls `window[go]()`.
 * `stop` is asked as it waits, and throws to give up (a Stop pressed meanwhile).
 */
async function held(switchName: string, go: string, stop: () => void = () => {}): Promise<boolean> {
  if (!new URLSearchParams(location.search).has(switchName)) return false;
  const w = window as unknown as Record<string, (() => void) | undefined>;
  let went = false;
  w[go] = () => {
    went = true;
  };
  try {
    while (!went) {
      await sleep(50);
      stop();
    }
  } finally {
    delete w[go];
  }
  return true;
}

/** How long a redraw takes: a moment, or with `?holdredraw` until the page calls `mockRedrawn()`,
 *  so a test can look at the library while it's drawn again, however busy the machine. */
async function redrawTime(): Promise<void> {
  if (!(await held("holdredraw", "mockRedrawn"))) await sleep(700);
}

export const mockApi = {
  folderLook: async (): Promise<FolderStyle> => (localStorage.getItem(MOCK_LOOK_KEY) === "windows" ? "windows" : "mac"),
  setFolderLook: async (look: FolderStyle): Promise<void> => {
    localStorage.setItem(MOCK_LOOK_KEY, look);
    mockRedraw = true;
  },
  listSkins: async (): Promise<SkinList> => {
    // The app draws every thumbnail again on the other folder, which takes a moment.
    if (mockRedraw) {
      mockRedraw = false;
      await redrawTime();
    }
    return {
      skins: [...library].sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0)),
      default_thumbnail: COLOUR_FOLDERS[0],
    };
  },
  inspectPath: async (path: string): Promise<PathInfo> => ({
    kind: isImagePath(path) ? "image" : "folder",
    name: path.split(/[\\/]/).pop() || path,
    path,
  }),
  importImage: async (path: string): Promise<Skin> => {
    await sleep(500);
    const name = (path.split(/[\\/]/).pop() || "Your picture").replace(/\.[^.]+$/, "");
    const skin: Skin = { id: `user:${Date.now()}`, name, collection: "yours", thumbnail: picture(library.length), custom: true, kind: "artwork", source: "import", created_at: Date.now(), tags: [] };
    keep([skin]);
    return skin;
  },
  applySkin: async (folder: string, skinId: string) => {
    await sleep(600);
    mockIcons.set(folder, library.find((s) => s.id === skinId)?.thumbnail ?? null);
  },
  revertSkin: async (folder: string) => {
    await sleep(400);
    mockIcons.set(folder, null);
  },
  // `?os=windows` or `?os=linux` draws the preview as that OS (Windows' own window buttons, and so on).
  platformInfo: async (): Promise<PlatformInfo> => {
    const os = new URLSearchParams(location.search).get("os");
    if (os === "windows") return { os, browse_label: "your PC", note: "browser preview: nothing is written to disk" };
    if (os === "linux") return { os, browse_label: "your computer", note: "browser preview: nothing is written to disk" };
    return { os: "macos", browse_label: "your Mac", note: "browser preview: nothing is written to disk" };
  },
  subfolderCount: async (folder: string): Promise<SubfolderCount> => {
    if (counting) counting.stopped = true;
    const c: Counting = { folder, found: 0, done: false, inside: new Map([[folder, 0]]), open: new Map(), read: new Set(), stopped: false };
    counting = c;
    void runCount(c);
    // A small folder is counted by the time the answer comes; a big one goes on.
    for (let waited = 0; waited < 80 && !c.done; waited += 10) await sleep(10);
    return countNow(c);
  },
  subfolderCounts: async (folder: string, paths: string[]): Promise<SubfolderCounts> => {
    const c = counting?.folder === folder ? counting : null;
    if (!c) return { folder, count: 0, done: false, paths: paths.map(() => ({ found: false, inside: 0, done: false })) };
    return {
      ...countNow(c),
      paths: paths.map((path) => {
        if (c.inside.has(path)) return { found: true, inside: c.inside.get(path) ?? 0, done: (c.open.get(path) ?? 1) === 0 };
        // Not come to yet, or not there at all: the folder it would be in was read without it.
        return { found: false, inside: 0, done: c.read.has(path.slice(0, path.lastIndexOf("/"))) || c.done };
      }),
    };
  },
  onSubfolderCount: (listener: (count: SubfolderCount) => void) => {
    countListeners.add(listener);
    return () => void countListeners.delete(listener);
  },
  subfolderList: async (folder: string): Promise<SubfolderList> => {
    await sleep(folder === BIG_TREE || folder === HUGE_TREE || folder === RUSHES ? 180 : 90);
    const names = mockChildren(folder);
    return { path: folder, separator: "/", names, nested: names.map((name) => mockChildren(`${folder}/${name}`).length > 0) };
  },
  treeBytes: async (_skinId: string, _folder?: string): Promise<number> => {
    await sleep(200);
    // What the app measured for a painted skin on macOS.
    return 2_670_631;
  },
  startTreeApply: async (folder: string, skinId: string, choice: RunChoice | null): Promise<TreeRunEvent> => {
    await sleep(40);
    const thumb = library.find((s) => s.id === skinId)?.thumbnail ?? null;
    const act = (path: string) => {
      if (path.endsWith("/Private")) return "you don't have permission to change it";
      if (path.startsWith(`${RUSHES}/Day 17 `) && !lockedOnce.has(path)) {
        lockedOnce.add(path);
        return "you don't have permission to change it";
      }
      mockTreeIcons.add(path);
      return "changed";
    };
    return mockStart("apply", folder, planOf(folder, choice), act, { skin_id: skinId }, thumb);
  },
  startTreeRevert: async (folder: string, choice: RunChoice | null, skipPlain: boolean): Promise<TreeRunEvent> => {
    await sleep(40);
    const act = (path: string) => {
      const has = mockTreeIcons.has(path) || (path === folder && mockIcons.get(folder) != null);
      if (skipPlain && !has) return "skipped";
      mockTreeIcons.delete(path);
      return "changed";
    };
    return mockStart("revert", folder, planOf(folder, choice), act, { leaves_plain: skipPlain }, null);
  },
  stopTreeRun: async (id: number) => {
    if (job?.run.id !== id || !job.run.running) return;
    job.stop = true;
    job.run.stopping = true;
    tellRun();
  },
  carryOnTreeRun: async (id: number): Promise<TreeRunEvent> => {
    const j = jobOf(id);
    if (j.run.running) throw new Error(`wait for the run in ${j.run.name} to finish, or stop it, before starting another`);
    void work(j);
    return tellRun();
  },
  retryTreeRun: async (id: number): Promise<TreeRunEvent> => {
    const j = jobOf(id);
    if (j.run.running) throw new Error(`wait for the run in ${j.run.name} to finish, or stop it, before starting another`);
    for (const [path, part] of j.parts) if (part === "failed") j.parts.set(path, "to do");
    j.run.done -= j.run.failed;
    j.run.failed = 0;
    j.run.failures = [];
    void work(j);
    return tellRun();
  },
  undoTreeRun: async (id: number): Promise<TreeRunEvent> => {
    const j = jobOf(id);
    if (j.run.kind !== "apply" || j.run.running) throw new Error("that run isn't there any more");
    const plan = j.plan.filter((path) => j.parts.get(path) === "changed");
    const act = (path: string) => {
      mockTreeIcons.delete(path);
      return "changed";
    };
    job = newJob("revert", j.run.folder, plan, act, { undoing: true, skin_id: j.run.skin_id, total: plan.length, counted: true }, null);
    void work(job);
    return tellRun();
  },
  dismissTreeRun: async (id: number) => {
    if (job?.run.id !== id || job.run.running) return;
    job = null;
    tellRun();
  },
  treeRun: async (): Promise<TreeRunEvent> => ({ seq: ++runSeq, run: job ? { ...job.run } : null }),
  onTreeRun: (listener: (event: TreeRunEvent) => void) => {
    runListeners.add(listener);
    return () => void runListeners.delete(listener);
  },
  // `?unfocused` has the window behind others, so a run that ends says so as a notification,
  // which the page keeps in `window.mockNotifications` for a test to read.
  windowFocused: async () => !new URLSearchParams(location.search).has("unfocused"),
  askToNotify: async () => true,
  notify: async (title: string, body: string) => {
    const w = window as unknown as { mockNotifications?: { title: string; body: string }[] };
    (w.mockNotifications ??= []).push({ title, body });
  },
  folderIcon: async (path: string): Promise<FolderIcon> => {
    await sleep(120);
    const custom = mockIcons.get(path) ?? null;
    return { url: custom ?? COLOUR_FOLDERS[0], custom: custom !== null };
  },
  skinsFolder: async () => "/Users/you/Library/Application Support/app.folderskin/skins",
  deleteSkin: async (skinId: string) => {
    library = library.filter((s) => s.id !== skinId);
  },
  editSkin: async (_skinId: string, name: string, tags: string[]) => ({ name: cleanName(name), tags: cleanTags(tags) }),
  communityPacks: async (_fresh = false): Promise<FirstPacks> => {
    await sleep(500);
    if (offline()) throw OFFLINE;
    const catalog = communityCatalog();
    // The sample packs haven't moved: they keep the ids they were first published under.
    return { packs: listed().map((p) => communityPack(catalog.find(p.id)!)), moved: {} };
  },
  communitySearch: async (query: CommunityQuery): Promise<CommunitySearch> => {
    // Counted where the end-to-end tests can read it, to see that coming back to Community
    // didn't search again.
    const counted = window as { mockCommunitySearches?: number };
    counted.mockCommunitySearches = (counted.mockCommunitySearches ?? 0) + 1;
    // The first search waits for the catalog, as the app's does; after that about an IPC round trip.
    await sleep(mockCatalogueLoaded ? 8 : 450);
    if (offline()) throw OFFLINE;
    mockCatalogueLoaded = true;
    const found = communityCatalog().search(query);
    const hitPacks = [...new Map(found.skins.map((h) => [h.pack.id, h.pack])).values()];
    return {
      total: found.total,
      all: found.all,
      packs: found.packs.map(communityPack),
      skins: found.skins.map((h) => ({ pack: h.pack.id, pack_name: h.pack.name, name: h.name, index: h.index, thumbnail: packPictures(h.pack)[h.index].thumbnail })),
      hit_packs: hitPacks.map(communityPack),
      facets: found.facets,
      last_visit: null,
      generation: "preview",
    };
  },
  communityRefresh: async (): Promise<{ updates: number; packs: number }> => {
    await sleep(600);
    if (offline()) throw OFFLINE;
    const packs = communityCatalog().packs;
    return { updates: packs.filter((p) => packAdded(p.id) && mockStale.has(p.id)).length, packs: packs.length };
  },
  communityPack: async (packId: string): Promise<CommunityPack | null> => {
    await sleep(mockCatalogueLoaded ? 8 : 450);
    if (offline()) throw OFFLINE;
    mockCatalogueLoaded = true;
    const pack = communityCatalog().find(packId);
    return pack ? communityPack(pack) : null;
  },
  takeInstallLink: async (): Promise<string | null> => {
    if (mockLinkTaken) return null;
    mockLinkTaken = true;
    return new URLSearchParams(location.search).get("install");
  },
  communityInstalled: async (): Promise<Record<string, string | null>> => {
    const installed: Record<string, string | null> = {};
    for (const skin of library) {
      if (!skin.pack || skin.pack in installed) continue;
      // A pack with a newer version out was added at some older one.
      installed[skin.pack] = mockStale.has(skin.pack) ? "0000000000000000" : (communityCatalog().find(skin.pack)?.hash ?? null);
    }
    return installed;
  },
  addPack: async (packId: string, onProgress?: (progress: PackProgress) => void): Promise<Skin[]> => {
    const pack = findPack(packId);
    if (!pack) throw "that isn't a pack";
    const total = pack.count;
    if (offline()) throw OFFLINE;
    onProgress?.({ stage: "download", done: 0, total });
    for (let done = 1; done <= total; done++) {
      await sleep(110);
      if (packId === "chrome-dreams" && !mockFailedOnce.has(packId) && done > total / 2) {
        mockFailedOnce.add(packId);
        throw OFFLINE;
      }
      onProgress?.({ stage: "download", done, total });
    }
    // `?holdpacks`: downloaded, it waits to be saved until the page calls `mockPackGo()`.
    await held("holdpacks", "mockPackGo");
    for (let done = 0; done <= total; done += 4) {
      onProgress?.({ stage: "save", done: Math.min(done, total), total });
      await sleep(80);
    }
    onProgress?.({ stage: "save", done: total, total });
    const skins = mockPackSkins(pack);
    keep(skins);
    if (packId === "colours" && !mockStale.has("colours-updated")) mockStale.add(packId);
    return skins;
  },
  packSkins: async (packId: string, _hash: string): Promise<PackSkinPreview[]> => {
    // Like the app's cache: slow the first time, straight away after.
    if (!mockViewed.has(packId)) await sleep(900);
    mockViewed.add(packId);
    const pack = findPack(packId);
    if (!pack) throw "that isn't a pack";
    return packPictures(pack).map((p) => ({ ...p, tags: pack.tags }));
  },
  updatePack: async (packId: string, onProgress?: (progress: PackProgress) => void): Promise<PackUpdate> => {
    const pack = findPack(packId);
    if (!pack) throw "that isn't a pack";
    for (let done = 0; done <= pack.count; done++) {
      onProgress?.({ stage: "download", done, total: pack.count });
      await sleep(1200 / Math.max(pack.count, 1));
    }
    await held("holdpacks", "mockPackGo");
    onProgress?.({ stage: "save", done: pack.count, total: pack.count });
    mockStale.delete(packId);
    mockStale.add(`${packId}-updated`);
    const skins = mockPackSkins(pack);
    keep(skins);
    return { removed: [], skins };
  },
  removePack: async (packId: string): Promise<string[]> => {
    const ids = library.filter((s) => s.pack === packId).map((s) => s.id);
    library = library.filter((s) => s.pack !== packId);
    return ids;
  },
  importPack: async (path: string): Promise<Skin[]> => {
    const id = (path.split(/[\\/]/).pop() || "my-pack").toLowerCase();
    const skins = mockPackSkins({ id, name: "Folder pack", author: "you", license: "CC0-1.0", tags: ["test"], count: 2 });
    keep(skins);
    return skins;
  },
  onboardingNeeded: async (): Promise<boolean> => {
    if (new URLSearchParams(location.search).has("onboarding")) return true;
    try {
      return localStorage.getItem(ONBOARDED_KEY) !== "1";
    } catch {
      return true;
    }
  },
  finishOnboarding: async () => {
    try {
      localStorage.setItem(ONBOARDED_KEY, "1");
    } catch {
      // The preview shows the onboarding again next time; nothing else depends on it.
    }
  },
  exportPack: async (req: ExportPackRequest, onProgress: (p: MakeProgress) => void): Promise<ExportedPack> => {
    await mockEncode(req.skinIds.length, onProgress);
    return { folder: `${req.folder}/${mockNewId(req.name)}`, scaled: mockScaled(req.skinIds) };
  },
  setWindowTheme: async () => {},
  // The browser has no menu bar and no system panels to put in another language.
  setLanguage: async (_language: string, _menu: Record<string, string>, _pin: boolean): Promise<void> => {},
  aiCatalogue: async (): Promise<AiCatalogue> => {
    // `?slowcatalogue`: as the app's first catalogue of a session, which asks the local runtime
    // whether it starts and takes a couple of seconds.
    if (new URLSearchParams(location.search).has("slowcatalogue")) await sleep(1500);
    await catalogueHeld;
    return mockProviders();
  },
  aiSetKey: async (provider: string) => {
    mockKeys.add(provider);
  },
  aiClearKey: async (provider: string) => {
    mockKeys.delete(provider);
  },
  aiTestKey: async () => {},
  composerTemplate: async (style: FolderStyle): Promise<ComposerTemplate> => ({ size: 1024, ...mockTemplateUrls(style), parts: fallbackParts(style) }),
  composerSave: async (header: ComposerSaveHeader, png: Uint8Array): Promise<ComposerSaved> => {
    await sleep(500);
    const old = header.replaces ? library.find((s) => s.id === header.replaces) : undefined;
    const skin: Skin = {
      id: `user:c${Date.now().toString(16)}`,
      name: cleanName(header.name) || "My design",
      collection: "yours",
      thumbnail: await mockIcon(png, header.shape, header.style, 512),
      custom: true,
      kind: "folder",
      source: "composer",
      created_at: old?.created_at ?? Date.now(),
      tags: cleanTags(header.tags),
    };
    mockDesigns.set(skin.id, header.design);
    if (old) {
      library = library.map((s) => (s.id === old.id ? skin : s));
      mockDesigns.delete(old.id);
    } else keep([skin]);
    return { skin, replaced: old ? old.id : null };
  },
  composerPreview: async (shape: "folder" | "free", style: FolderStyle, sizes: number[], png: Uint8Array): Promise<string[]> =>
    Promise.all(sizes.map((size) => mockIcon(png, shape, style, size))),
  composerImage: async (_path: string): Promise<ComposerImage> => {
    await sleep(250);
    return mockPhoto();
  },
  composerSkinImage: async (skinId: string): Promise<ComposerImage> => {
    const skin = library.find((s) => s.id === skinId);
    if (!skin) throw "that skin isn't available any more";
    const img = await loadImg(skin.thumbnail);
    const c = document.createElement("canvas");
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    c.getContext("2d")!.drawImage(img, 0, 0);
    return { url: c.toDataURL("image/png"), width: c.width, height: c.height, name: skin.name, alpha: true };
  },
  composerDesign: async (skinId: string): Promise<unknown> => mockDesigns.get(skinId) ?? null,
  aiGenerate: async (req: AiGenerateRequest, onEvent: (event: AiEvent) => void = () => {}): Promise<Skin> => {
    const local = req.provider === "local";
    const who = local ? "the local model" : (MOCK_LABELS[req.provider] ?? req.provider);
    const job = req.job ?? "";
    const stop = () => {
      if (mockStopped.has(job)) throw { code: "stopped", message: "Stopped before it finished." };
    };
    // Waits `ms`, or gives up the moment the run is stopped.
    const wait = async (ms: number) => {
      for (let t = 0; t < ms; t += 100) {
        stop();
        await sleep(100);
      }
    };
    // `?holdpaint`: painted, the picture waits until the page calls `mockPaintGo()`.
    const painted = () => held("holdpaint", "mockPaintGo", stop);
    const fail = new URLSearchParams(location.search).get("aifail");
    // The errors are the objects ai/failure.rs returns, with its codes and words.
    if (local) {
      if (!mockLocal.ready) {
        throw { code: "local_not_ready", message: "Skins can't be generated with the local model until it's set up.", fix: ["The models aren't downloaded yet. Set it up in the provider settings."] };
      }
      onEvent({ type: "log", level: "info", message: "backend: CUDA (NVIDIA GeForce RTX 3050 Ti, 4 GB)" });
      onEvent({ type: "log", level: "info", message: "model: FLUX.2 [klein] 4B, q4 weights" });
      onEvent({ type: "stage", stage: "load", message: "Loading the model" });
      await wait(500);
      onEvent({ type: "stage", stage: "paint", message: "Painting" });
      for (let step = 1; step <= 4; step++) {
        await wait(520);
        onEvent({ type: "progress", step, steps: 4 });
        onEvent({ type: "log", level: "info", message: `[INFO ] sampling step ${step}/4, 0.9 s/it` });
      }
      await painted();
      if (fail === "memory") {
        throw {
          code: "out_of_memory",
          message: "The graphics card ran out of memory while painting.",
          fix: [
            "Close apps that use the graphics card, such as games or video editors, then try again.",
            "If it keeps happening, restart the computer: something may still be holding the graphics card's memory.",
          ],
          ask: `claude "On windows x86_64 with FolderSkin 0.1.3, painting ${req.shape === "folder" ? "a whole folder" : "folder artwork"} with the local model (CUDA, NVIDIA GeForce RTX 3050 Ti, 4 GB) failed with out_of_memory: The graphics card ran out of memory while painting. stable-diffusion.cpp stopped with exit code 1. Help me fix it."`,
        };
      }
      // Painted: now the settings can say how long a picture takes here (ai/local.rs Timing).
      mockLocal.seconds = 18;
    } else {
      onEvent({ type: "stage", stage: "send", message: `Sending your idea to ${who}` });
      await wait(700);
      if (fail === "key") throw { code: "missing_key", message: `Add your ${who} API key first.` };
      if (fail === "refused") throw { code: "refused", message: `${who} declined that prompt: it asks for something its safety system won't draw.` };
      if (fail === "rate") throw { code: "rate_limited", message: `${who} is rate limiting you right now. Wait a moment and try again.` };
      if (fail === "network") {
        throw { code: "network", message: `Couldn't reach ${who}: the connection was refused.`, fix: ["Check the internet connection (and any proxy or firewall), then try again."] };
      }
      onEvent({ type: "stage", stage: "paint", message: `${who} is painting it` });
      await wait(1800);
      if (fail === "noimage") throw { code: "no_image", message: `${who} finished without painting a picture. Try again, or reword the idea.` };
      await painted();
    }
    // As ai.rs: a free icon is always cut out and used as it is, whatever the shape asked says.
    const free = req.base === "free";
    if (req.shape === "folder" || free) {
      onEvent({ type: "stage", stage: "cut", message: "Cutting it out of the background" });
      await wait(400);
    }
    onEvent({ type: "stage", stage: "save", message: "Saving it to Yours" });
    await wait(200);
    // The look picked goes in its own slot, and the picture is tagged with its style as ai.rs tags
    // it: a built-in style's, or the style a saved prompt's look uses.
    const skill = req.skill ? mockPrompts().find((p) => p.id === req.skill) : undefined;
    const style = styleById(skill?.base_style ?? req.style ?? null);
    const skin: Skin = {
      id: `user:ai${Date.now().toString(16)}`,
      name: mockShortName(req.idea),
      collection: "yours",
      thumbnail: free ? mockIconPicture(req.idea) : picture(library.length + 1),
      custom: true,
      kind: req.shape === "folder" || free ? "folder" : "artwork",
      source: "ai",
      created_at: Date.now(),
      tags: cleanTags([...req.tags, ...(style ? [style.tag] : [])]),
      made_with: local ? "Local Model · FLUX.2 klein 4B" : `${who} · ${req.model}`,
      idea: req.idea,
      base: req.base ?? "mac-folder",
    };
    keep([skin]);
    return skin;
  },
  aiCancel: async (job: string): Promise<void> => {
    mockStopped.add(job);
  },
  aiLocalStatus: async (): Promise<LocalStatus> => {
    // Once the runtime is installed, ai_local_status asks it whether it starts, which takes a moment.
    const [runtime, size] = MOCK_SETUP_FILES[0];
    if (mockLocal.ready || mockLocal.got[runtime] === size) await sleep(500);
    return mockLocalStatus();
  },
  aiLocalRemoveUnused: async (): Promise<LocalStatus> => {
    if (mockSetup) throw { code: "busy", message: "The local model is being set up.", fix: ["Stop the setup, then remove the files."] };
    await sleep(300);
    mockLocal.unused = 0;
    return mockLocalStatus();
  },
  aiLocalRemove: async (): Promise<LocalStatus> => {
    // As ai_local_remove: refused while it's being set up; otherwise what was downloaded goes.
    if (mockSetup) throw { code: "busy", message: "The local model is being set up.", fix: ["Stop the setup, then remove the model."] };
    await sleep(300);
    // How long a picture took here stays: it belongs to the machine, not to the files.
    mockLocal.ready = false;
    mockLocal.unused = 0;
    mockLocal.got = {};
    return mockLocalStatus();
  },
  aiLocalSetup: async (onEvent: (event: AiEvent) => void): Promise<LocalStatus> => {
    // Asked while one runs (the panel was closed and opened again), it joins that one.
    if (mockSetup) {
      for (const event of mockSetup.heard()) onEvent(event);
      mockSetup.listeners.push(onEvent);
      return mockSetup.done;
    }
    // aiCancel(LOCAL_SETUP_JOB) stops it, as ai_local_setup does; a Stop from an earlier setup doesn't count.
    const SETUP = "local-setup";
    mockStopped.delete(SETUP);
    const listeners = [onEvent];
    let stage: AiEvent | null = null;
    let download: AiEvent | null = null;
    const log: AiEvent[] = [];
    const tell = (event: AiEvent) => {
      // A new stage is about something else: the file before it isn't heard again (ai/local.rs).
      if (event.type === "stage") [stage, download] = [event, null];
      else if (event.type === "download") download = event;
      else if (event.type === "log") log.push(event);
      for (const listener of listeners) listener(event);
    };
    const stopped = () => ({ code: "stopped", message: "Stopped. What was downloaded is kept, and setting up again carries on from there." });
    // `?holdsetup`: the setup waits once it has downloaded everything, and with `?holdsetup=n`
    // after its first n pieces too, until the page calls `mockSetupGo()`, so a test finds it
    // under way, and where it expects, however busy the machine. Stop still stops it there.
    const holding = new URLSearchParams(location.search).get("holdsetup");
    const holdAfter = holding === null ? null : Number(holding);
    const hold = () =>
      held("holdsetup", "mockSetupGo", () => {
        if (mockStopped.delete(SETUP)) throw stopped();
      });
    const run = async (): Promise<LocalStatus> => {
      tell({ type: "stage", stage: "download", message: "Downloading what the local model needs" });
      let pieces = 0;
      for (const [file, total] of MOCK_SETUP_FILES) {
        // Where it starts from first, as download.rs says before the first chunk: where the last
        // setup left it.
        const from = mockLocal.got[file] ?? 0;
        tell({ type: "download", file, done: from, total });
        for (let i = 1; i <= 5 && from < total; i++) {
          await sleep(90);
          if (mockStopped.delete(SETUP)) throw stopped();
          mockLocal.got[file] = from + Math.round(((total - from) * i) / 5);
          tell({ type: "download", file, done: mockLocal.got[file], total });
          if (++pieces === holdAfter) await hold();
        }
        tell({ type: "log", level: "info", message: `checked ${file}` });
      }
      if (holdAfter !== null) await hold();
      tell({ type: "stage", stage: "check", message: "Checking it runs" });
      await sleep(400);
      mockLocal.ready = true;
      mockSetup = null;
      return mockLocalStatus();
    };
    const done = run().finally(() => {
      mockSetup = null;
    });
    mockSetup = { listeners, heard: () => [...log, ...(stage ? [stage] : []), ...(download ? [download] : [])], done };
    return done;
  },
  chatsList: async (): Promise<ChatSummaryDto[]> => mockChats().index,
  chatRead: async (id: string): Promise<unknown> => {
    const chat = mockChats().chats[id];
    if (!chat) throw "that chat isn't on this computer any more";
    return chat;
  },
  chatSave: async (chat: unknown): Promise<ChatSummaryDto> => {
    const c = chat as { id: string; title: string; created: number; updated: number; turns: { skinId?: string }[] };
    const store = mockChats();
    const summary: ChatSummaryDto = {
      id: c.id,
      title: c.title,
      created: c.created,
      updated: c.updated,
      turns: c.turns.length,
      pictures: c.turns.filter((t) => t.skinId).length,
      cover: [...c.turns].reverse().find((t) => t.skinId)?.skinId ?? null,
    };
    store.chats[c.id] = chat;
    store.index = [summary, ...store.index.filter((s) => s.id !== c.id)].sort((a, b) => b.updated - a.updated);
    saveMockChats(store);
    return summary;
  },
  chatDelete: async (id: string): Promise<void> => {
    const store = mockChats();
    delete store.chats[id];
    store.index = store.index.filter((s) => s.id !== id);
    saveMockChats(store);
  },
  chatKeepReference: async (_id: string, path: string): Promise<ChatRefDto> => {
    await sleep(150);
    const name = path.split(/[\\/]/).pop() || "picture.jpg";
    return { id: Math.random().toString(16).slice(2, 14), name, path, thumb: picture(3) };
  },
  aiShapes: async (): Promise<ShapeInfo[]> => mockShapes(),
  promptsList: async (): Promise<SavedPrompt[]> => {
    // `?promptsfail`: the list can't be read, as a damaged file or a missing data folder would.
    if (new URLSearchParams(location.search).has("promptsfail")) throw "prompts can't be kept on this computer: FolderSkin has no data folder here";
    await sleep(60);
    return mockPrompts();
  },
  promptSave: async (name: string, text: string, style: string | null): Promise<SavedPrompt> => {
    await sleep(120);
    const clean = name.replace(/\s+/g, " ").trim();
    if (!clean) throw "give the prompt a name";
    if (clean.length > 60) throw "that name is too long. Keep it to 60 characters";
    const list = mockPrompts();
    // As prompts.rs saves it: a built-in style by id, or another saved prompt's look, whole.
    const from = style ? list.find((p) => p.id === style) : undefined;
    const look = from
      ? { base_style: from.base_style, treatment: from.treatment, light: from.light, palette: from.palette, keep_out: from.keep_out, lettering: from.lettering }
      : { base_style: style ? (styleById(style)?.id ?? style) : null };
    if (!text.trim() && !look.base_style && !("treatment" in look && look.treatment)) throw "there's nothing to save yet. Write the prompt first";
    const same = list.find((p) => p.name.toLowerCase() === clean.toLowerCase());
    const now = new Date().toISOString().replace(/\.\d+Z$/, "Z");
    const command = clean.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "prompt";
    const saved: SavedPrompt = {
      format: "folderskin.skill/1",
      id: same?.id ?? `${command}-${Math.random().toString(36).slice(2, 6)}`,
      name: clean,
      command,
      idea: text.trim() || null,
      ...look,
      created: same?.created ?? now,
      updated: now,
    };
    saveMockPrompts(same ? list.map((p) => (p.id === same.id ? saved : p)) : [saved, ...list]);
    return saved;
  },
  promptRestore: async (skill: SavedPrompt, at: number | null): Promise<SavedPrompt> => {
    await sleep(80);
    const list = mockPrompts();
    const kept = list.find((p) => p.id === skill.id || p.name.toLowerCase() === skill.name.toLowerCase());
    if (kept) return kept;
    list.splice(Math.min(at ?? 0, list.length), 0, skill);
    saveMockPrompts(list);
    return skill;
  },
  promptDelete: async (id: string): Promise<void> => {
    await sleep(80);
    saveMockPrompts(mockPrompts().filter((p) => p.id !== id));
  },
  iconPackDownload: async (id: string, _release: string, _sha256: string, bytes: number, onProgress: (p: IconPackProgress) => void): Promise<void> => {
    if (offline()) {
      await sleep(400);
      throw "couldn't download the icon pack. Check your connection and try again";
    }
    // The real pack when scripts/icon-packs.mjs --all has built it (Vite serves dist-icons/ in
    // dev); otherwise Lucide under the pack's name, so the whole flow works without a build.
    let text = await fetch(`/dist-icons/${id}.json`).then((r) => (r.ok ? r.text() : null)).catch(() => null);
    if (!text || !text.startsWith("{")) {
      const lucide = (await import("../composer/icons/lucide.json")).default as Record<string, unknown>;
      text = JSON.stringify({ ...lucide, id, name: id });
    }
    for (let i = 1; i <= 8; i++) {
      await sleep(90);
      onProgress({ done: Math.round((bytes * i) / 8), total: bytes });
    }
    mockIconPacks.set(id, text);
  },
  iconPacksInstalled: async (): Promise<InstalledIconPack[]> => [...mockIconPacks.entries()].map(([id, text]) => ({ id, sha256: "", bytes: text.length })),
  iconPackRead: async (id: string): Promise<string> => {
    const text = mockIconPacks.get(id);
    if (!text) throw "that icon pack isn't downloaded";
    return text;
  },
  iconPackRemove: async (id: string): Promise<void> => {
    mockIconPacks.delete(id);
  },
  shareStatus: async (): Promise<ShareStatus> => {
    await sleep(300);
    return mockShareStatus();
  },
  shareVerify: async (handle: string): Promise<string> => {
    await sleep(200);
    if (!/^[A-Za-z0-9]+(-[A-Za-z0-9]+)*$/.test(handle) || handle.length < 3 || handle.length > 39) {
      throw "A name is 3 to 39 letters, digits and single dashes, not starting or ending with a dash.";
    }
    mockShare.key = true;
    mockShare.wanted = handle;
    mockShare.waiting = true;
    return `https://community.example.org/verify?h=${handle}`;
  },
  shareWait: async (): Promise<ShareStatus> => {
    // As long as the check in the browser takes, give or take.
    for (let waited = 0; waited < 1800; waited += 100) {
      await sleep(100);
      if (!mockShare.waiting) throw "Verifying was stopped.";
    }
    mockShare.waiting = false;
    mockShare.handle = mockShare.handle ?? mockShare.wanted;
    return mockShareStatus();
  },
  shareCancel: async () => {
    mockShare.waiting = false;
  },
  shareSaveKey: async (_path: string) => {
    await sleep(200);
    if (!mockShare.key) throw "This computer has no sharing key yet.";
  },
  shareLoadKey: async (_path: string): Promise<ShareStatus> => {
    await sleep(300);
    mockShare.key = true;
    mockShare.handle = "sunny-otter";
    return mockShareStatus();
  },
  shareSubmit: async (pack: PackToShare, onProgress: (p: ShareProgress) => void): Promise<SharedPack> => {
    if (!mockShare.handle) throw "Verify this computer first, so the service knows the pack is yours.";
    if (pack.termsVersion !== MOCK_TERMS_VERSION) throw "The pack terms have changed. Update FolderSkin, read them and send the pack again.";
    onProgress({ stage: "preparing" });
    await sleep(300);
    await mockEncode(pack.skinIds.length, (p) => onProgress({ stage: "encoding", ...p }));
    onProgress({ stage: "checking" });
    await sleep(400);
    const params = new URLSearchParams(location.search);
    if (params.has("cooling")) throw SHARE_COOLING;
    const total = pack.skinIds.length;
    for (let done = 0; done <= total; done++) {
      onProgress({ stage: "uploading", done, total });
      await sleep(90);
      // The service's limit on bursts turns the first picture down; the app waits and sends it again.
      if (done === 0 && params.has("slowdown")) {
        onProgress({ stage: "waiting", seconds: 2 });
        await sleep(2000);
        onProgress({ stage: "uploading", done, total });
      }
    }
    onProgress({ stage: "finishing" });
    await sleep(500);
    const shared: SharedPack = {
      submission_id: `sub_mock${Date.now().toString(36)}`,
      name: cleanName(pack.name),
      pictures: total,
      scaled: mockScaled(pack.skinIds),
    };
    mockShare.submissions.unshift({
      id: shared.submission_id,
      name: shared.name,
      status: "in_review",
      pictures: total,
      license: pack.license,
      created_at: Math.floor(Date.now() / 1000),
      decided_at: null,
      pack_id: null,
      pulled: false,
      reasons: [],
      note: "",
    });
    return shared;
  },
  shareSubmissions: async (): Promise<MySubmission[]> => {
    await sleep(400);
    seedShared();
    if (offline()) throw SHARE_UNREACHABLE;
    return mockShare.submissions.map((s) => ({ ...s }));
  },
  shareWithdraw: async (id: string) => {
    await sleep(300);
    mockShare.submissions = mockShare.submissions.map((s) => (s.id === id ? { ...s, status: "withdrawn", pack_id: null } : s));
  },
};

/** `?holdcatalogue`: the providers don't come until the page calls `mockCatalogueIn()`, so a test
 *  can look at the AI view while they load, however busy the machine. */
const catalogueHeld = new URLSearchParams(location.search).has("holdcatalogue")
  ? new Promise<void>((resolve) => ((window as { mockCatalogueIn?: () => void }).mockCatalogueIn = resolve))
  : Promise.resolve();

/** A model as ai_catalogue lists it: how many pictures it takes, and its price. */
const mockModel = (id: string, label: string, max_references: number, price_hint: string, native_alpha = false): AiModel => ({
  id,
  label,
  native_alpha,
  accepts_reference: max_references > 0,
  max_references,
  sizes: ["1024x1024"],
  price_hint,
});

/** The providers and models, as ai_catalogue lists them (crates/folderskin-ai/src/catalogue.rs), with the keys saved in this preview. */
function mockProviders(): AiCatalogue {
  const provider = (id: string, label: string, models: AiModel[], keys_url: string, docs_url: string, key_hint: string): AiProvider => ({
    id,
    label,
    kind: "key",
    models,
    keys_url,
    docs_url,
    key_hint,
    has_key: mockKeys.has(id),
  });
  return {
    providers: [
      {
        id: "local",
        label: "Local Model",
        kind: "local",
        models: [mockModel("klein", "FLUX.2 klein 4B", 4, "Free")],
        keys_url: "",
        docs_url: "",
        key_hint: "",
        has_key: mockLocal.ready,
      },
      provider(
        "openai",
        "OpenAI",
        [
          mockModel("gpt-image-2.5-flare", "GPT Image 2.5 Flare", 16, "~$0.05 / image", true),
          mockModel("gpt-image-2.5-sunburst", "GPT Image 2.5 Sunburst", 16, "~$0.20 / image", true),
          mockModel("gpt-image-2", "GPT Image 2", 16, "~$0.05 / image"),
        ],
        "https://platform.openai.com/api-keys",
        "https://developers.openai.com/api/docs/guides/image-generation",
        "starts with sk-",
      ),
      provider(
        "xai",
        "xAI Grok",
        [mockModel("grok-imagine-image-2.0", "Grok Imagine 2.0", 5, "~$0.04 / image"), mockModel("grok-imagine-image", "Grok Imagine", 3, "~$0.02 / image")],
        "https://console.x.ai",
        "https://docs.x.ai/developers/model-capabilities/imagine",
        "starts with xai-",
      ),
      provider(
        "recraft",
        "Recraft",
        [mockModel("recraftv4_1", "Recraft V4.1", 0, "~$0.035 / image"), mockModel("recraftv4_1_flash", "Recraft V4.1 Flash", 0, "~$0.007 / image")],
        "https://app.recraft.ai/profile/api",
        "https://www.recraft.ai/docs",
        "from your Recraft profile",
      ),
      provider(
        "google",
        "Google Gemini",
        [
          mockModel("gemini-3.1-flash-image", "Gemini 3.1 Flash Image", 14, "~$0.07 / image"),
          mockModel("gemini-3-pro-image", "Gemini 3 Pro Image", 14, "~$0.14 / image"),
          mockModel("gemini-3.1-flash-lite-image", "Gemini 3.1 Flash Lite Image", 14, "~$0.034 / image"),
        ],
        "https://aistudio.google.com/apikey",
        "https://ai.google.dev/gemini-api/docs/generate-content/image-generation",
        "from Google AI Studio",
      ),
      provider(
        "bfl",
        "Black Forest Labs",
        [
          mockModel("flux-2-pro", "FLUX.2 pro", 8, "~$0.03–0.05 / image"),
          mockModel("flux-2-max", "FLUX.2 max", 8, "~$0.07–0.10 / image"),
          mockModel("flux-2-flex", "FLUX.2 flex", 8, "~$0.05–0.10 / image"),
          mockModel("flux-2-klein-4b", "FLUX.2 klein 4B", 4, "~$0.015 / image"),
        ],
        "https://dashboard.bfl.ai",
        "https://docs.bfl.ai",
        "from the BFL dashboard",
      ),
      provider(
        "stability",
        "Stability AI",
        [mockModel("ultra", "Stable Image Ultra", 0, "~$0.08 / image"), mockModel("core", "Stable Image Core", 0, "~$0.03 / image")],
        "https://platform.stability.ai/account/keys",
        "https://platform.stability.ai/docs/api-reference",
        "starts with sk-",
      ),
      provider("ideogram", "Ideogram", [mockModel("V_3", "Ideogram 3.0", 0, "~$0.06 / image")], "https://ideogram.ai/manage-api", "https://developer.ideogram.ai", "from your Ideogram account"),
    ],
    // As folderskin_ai::prompts::PRESETS has them.
    presets: [
      { id: "aurora", label: "Aurora", idea: "a night sky with green and violet aurora ribbons over dark mountains, faint stars" },
      { id: "dunes", label: "Dunes", idea: "warm desert dunes at golden hour, long soft shadows, fine sand grain" },
      { id: "lighthouse", label: "Lighthouse", idea: "a lighthouse on a rocky point at dusk, its beam sweeping over a calm sea" },
      { id: "terrazzo", label: "Terrazzo", idea: "pale terrazzo with scattered chips of teal, ochre and charcoal" },
      { id: "wave", label: "Wave", idea: "a towering ocean wave with deep indigo troughs and white foam" },
      { id: "circuit", label: "Circuit", idea: "an emerald circuit board macro, gold traces, soft bokeh highlights" },
      { id: "linen", label: "Linen", idea: "undyed linen weave in raking light, visible slubs and thread texture" },
      { id: "nebula", label: "Nebula", idea: "a violet and cyan nebula with dust lanes and scattered stars" },
    ],
  };
}
