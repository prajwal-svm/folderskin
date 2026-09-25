/**
 * The words in a pack: the pack.json rules they follow (the same as folderskin_core::pack), and
 * the checks for words that need a person to look before anything goes further.
 */
import { englishDataset, englishRecommendedTransformers, RegExpMatcher } from "obscenity";
import {
  ID_BASE_CHARS,
  ID_SUFFIX_CHARS,
  LICENSES,
  MAX_PACK_ID_CHARS,
  MAX_PACK_NAME_CHARS,
  MAX_PACK_TAGS,
  MAX_SKIN_NAME_CHARS,
  MAX_SKIN_TAGS,
  MAX_SKINS,
  MAX_TAG_CHARS,
  PICTURE_EXTENSIONS,
} from "./limits";

/** A pack as the app sends it: everything but the fields the service decides (version, author, licence). */
export type Manifest = { name: string; tags: string[]; skins: { file: string; name: string; tags: string[] }[] };

/** Something a person should look at, and how soon. `detail` is for the maintainer only. */
export type Flag = { code: string; severity: "normal" | "high"; detail: string };

// ---- the pack.json rules ----

/** Characters that don't show but change what is shown: controls, and the marks that flip text direction. */
const HIDDEN = /[\p{Cc}\u061C\u200E\u200F\u202A-\u202E\u2066-\u2069]/u;

export function hasText(value: unknown, max: number): value is string {
  if (typeof value !== "string") return false;
  const s = value.trim();
  return s.length > 0 && [...s].length <= max && !HIDDEN.test(s);
}

/** Whether `text` has a character that doesn't show; with `lines`, a line break (\n) doesn't count as one. */
export function hasHidden(text: string, { lines = false } = {}): boolean {
  return HIDDEN.test(lines ? text.replace(/\n/g, "") : text);
}

/** A tag as FolderSkin keeps it (`pack::clean_tag`): lower case, letters, digits, single spaces and dashes. */
export function cleanTag(tag: string): string | null {
  const kept = [...tag].filter((c) => /[\p{Alphabetic}\p{N}-]/u.test(c) || /\s/u.test(c)).join("").toLowerCase();
  const words = kept.split(/\s+/u).filter(Boolean).join(" ");
  const cut = [...words].slice(0, MAX_TAG_CHARS).join("").replace(/^[ -]+|[ -]+$/g, "");
  return cut ? cut : null;
}

export function isWindowsDeviceName(name: string): boolean {
  const stem = name.split(".")[0].toLowerCase();
  return /^(con|prn|aux|nul|com\d|lpt\d)$/.test(stem);
}

/** A picture's file name in a pack (`pack::is_picture_file_name`). */
export function isPictureFileName(file: unknown): file is string {
  if (typeof file !== "string") return false;
  const ext = file.includes(".") ? file.slice(file.lastIndexOf(".") + 1).toLowerCase() : "";
  return (
    file.length > 0 &&
    file.length <= 64 &&
    !file.startsWith(".") &&
    !isWindowsDeviceName(file) &&
    /^[A-Za-z0-9._-]+$/.test(file) &&
    (PICTURE_EXTENSIONS as readonly string[]).includes(ext)
  );
}

/** A GitHub user name's shape (`pack::is_github_user`), which pack.json v1 needs of an author. */
export function isGithubUser(name: unknown): name is string {
  return typeof name === "string" && name.length <= 39 && /^[A-Za-z0-9]+(-[A-Za-z0-9]+)*$/.test(name);
}

/** Names nobody can take, so no pack looks like it comes from FolderSkin itself. */
const RESERVED = new Set([
  "admin",
  "administrator",
  "folderskin",
  "folder-skin",
  "moderator",
  "mod",
  "official",
  "root",
  "staff",
  "support",
  "system",
  "team",
  "prajwal-svm",
]);

/**
 * Why a name can't be a handle, or `null` when it can. Handles are credited as a pack's author, so
 * they follow the same shape a GitHub user name does.
 */
export function handleProblem(name: unknown): string | null {
  if (!isGithubUser(name) || name.length < 3) {
    return "A name is 3 to 39 letters, digits and single dashes, not starting or ending with a dash.";
  }
  if (RESERVED.has(name.toLowerCase()) || name.toLowerCase().startsWith("folderskin")) {
    return "That name is kept for FolderSkin itself. Choose another.";
  }
  if (textFlags([{ label: "name", text: name.replace(/-/g, " ") }]).length > 0) {
    return "That name isn't allowed. Choose another.";
  }
  return null;
}

/** A pack's folder name (`pack::is_pack_id`): lower-case letters and digits in words joined by single dashes, at most 40 characters. */
export function isPackId(id: unknown): id is string {
  return typeof id === "string" && id.length <= MAX_PACK_ID_CHARS && /^[a-z0-9]+(-[a-z0-9]+)*$/.test(id) && !isWindowsDeviceName(id);
}

/** A pack's name as a pack id would start (`pack::slug`): `"Ukiyo-e Nights!"` is `"ukiyo-e-nights"`. */
export function slug(name: string): string {
  let out = "";
  for (const c of name.toLowerCase()) {
    if (/[a-z0-9]/.test(c)) out += c;
    else if (out && !out.endsWith("-")) out += "-";
  }
  const cut = out.replace(/-+$/, "").slice(0, 40).replace(/-+$/, "");
  return isWindowsDeviceName(cut) ? `${cut}-1` : cut;
}

/** What a generated id's last six characters are drawn from: lower-case base32, like a submission's id. */
const ID_ALPHABET = "abcdefghijklmnopqrstuvwxyz234567";
/** How many ids `newPackId` draws before it gives up, which only a `taken` that says yes to everything makes it do. */
const ID_ATTEMPTS = 100;

/**
 * A new id for a pack called `name` (`pack::new_id`): the name's slug cut to 33 characters (or
 * "pack" when it has no letters or digits), a dash, and six characters drawn at random, such as
 * `classic-art-k7q2mx`. Names can repeat as often as they like; ids never do, since an id `taken`
 * says is in use is drawn again. With 2^30 endings for each name that practically never happens,
 * but it is checked all the same. An id never changes once a pack has it, even if the pack is
 * renamed.
 */
export async function newPackId(name: string, taken: (id: string) => boolean | Promise<boolean>): Promise<string> {
  const base = slug(name).slice(0, ID_BASE_CHARS).replace(/-+$/, "") || "pack";
  for (let attempt = 0; attempt < ID_ATTEMPTS; attempt++) {
    // 256 is a multiple of 32, so the low five bits of a random byte are uniform: every character
    // of the alphabet is exactly as likely as the others, with no modulo bias.
    let suffix = "";
    for (const byte of crypto.getRandomValues(new Uint8Array(ID_SUFFIX_CHARS))) suffix += ID_ALPHABET[byte & 31];
    const id = `${base}-${suffix}`;
    if (isPackId(id) && !(await taken(id))) return id;
  }
  throw new Error(`no free pack id for ${base} in ${ID_ATTEMPTS} tries`);
}

const GENERATED_ID = /^[a-z0-9]+(?:-[a-z0-9]+)*-[a-z2-7]{6}$/;

/**
 * Whether `id` has the shape `newPackId` gives (`pack::is_generated_id`). It is a shape and no
 * more: an id from before ids were generated can happen to have it, as `night-prints-bright` does.
 */
export function isGeneratedId(id: unknown): id is string {
  return typeof id === "string" && id.length <= MAX_PACK_ID_CHARS && GENERATED_ID.test(id) && isPackId(id);
}

/**
 * `text` without the slashes it ends with. Walked back from the end rather than `/\/+$/`, which
 * tries every run of slashes in the text and takes quadratic time on a long one that isn't last.
 */
export function trimTrailingSlashes(text: string): string {
  let end = text.length;
  while (end > 0 && text[end - 1] === "/") end--;
  return text.slice(0, end);
}

function tagProblems(tags: unknown, max: number, whose: string, problems: string[]): string[] {
  if (!Array.isArray(tags)) {
    problems.push(`${whose} needs a list of tags`);
    return [];
  }
  if (tags.length > max) problems.push(`${whose} has ${tags.length} tags; the most is ${max}`);
  const seen: string[] = [];
  for (const tag of tags) {
    const clean = typeof tag === "string" ? cleanTag(tag) : null;
    if (clean === null || clean !== tag) problems.push(`${whose}: ${JSON.stringify(tag)} isn't a tag as FolderSkin writes them`);
    else if (seen.includes(clean)) problems.push(`${whose}: the tag ${JSON.stringify(tag)} is listed twice`);
    else seen.push(clean);
  }
  return seen;
}

/**
 * Reads the pack the app sent, holding it to the pack.json rules. Returns the pack as it will be
 * kept, or every problem with it, one phrase each.
 */
export function readManifest(value: unknown): { manifest: Manifest } | { problems: string[] } {
  const problems: string[] = [];
  if (!value || typeof value !== "object") return { problems: ["the pack is missing"] };
  const input = value as Record<string, unknown>;
  if (!hasText(input.name, MAX_PACK_NAME_CHARS)) problems.push(`the name must be 1 to ${MAX_PACK_NAME_CHARS} characters`);
  else if (!slug(input.name as string)) problems.push("the name needs letters or digits in it");
  const tags = tagProblems(input.tags, MAX_PACK_TAGS, "the pack", problems);
  if (Array.isArray(input.tags) && input.tags.length === 0) problems.push("the pack needs at least one tag");
  const skins: Manifest["skins"] = [];
  const list = Array.isArray(input.skins) ? input.skins : [];
  if (list.length === 0 || list.length > MAX_SKINS) problems.push(`a pack has 1 to ${MAX_SKINS} skins`);
  const files = new Set<string>();
  for (const [i, raw] of list.slice(0, MAX_SKINS).entries()) {
    const skin = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;
    const label = `skin ${i + 1}`;
    if (!isPictureFileName(skin.file)) problems.push(`${label}: the file name isn't one a pack can use`);
    else if (files.has(skin.file.toLowerCase())) problems.push(`${label}: ${skin.file} is listed twice`);
    else files.add(skin.file.toLowerCase());
    if (!hasText(skin.name, MAX_SKIN_NAME_CHARS)) problems.push(`${label}: the name must be 1 to ${MAX_SKIN_NAME_CHARS} characters`);
    const own = tagProblems(skin.tags ?? [], MAX_SKIN_TAGS, label, problems);
    skins.push({ file: String(skin.file), name: String(skin.name ?? "").trim(), tags: own });
  }
  if (problems.length > 0) return { problems };
  return { manifest: { name: (input.name as string).trim(), tags, skins } };
}

export const isLicense = (value: unknown): value is (typeof LICENSES)[number] =>
  typeof value === "string" && (LICENSES as readonly string[]).includes(value);

// ---- words a person should see first ----

/**
 * Words tied to what the pack terms forbid outright (rules 6, 7, 9 and 8): sexual content, anything
 * sexualising a child, hate symbols and self-harm communities. A hit doesn't turn a pack away by
 * itself, since "naked" can be a mole rat, but it goes to the maintainer at once.
 */
const BLOCKLIST = [
  "loli",
  "lolicon",
  "shota",
  "shotacon",
  "jailbait",
  "pedo",
  "paedo",
  "pedophile",
  "preteen",
  "underage",
  "child porn",
  "cp",
  "csam",
  "nsfw",
  "porn",
  "porno",
  "hentai",
  "xxx",
  "nude",
  "nudes",
  "naked",
  "onlyfans",
  "rule34",
  "r34",
  "swastika",
  "kkk",
  "heil",
  "1488",
  "14 88",
  "white power",
  "proana",
  "pro ana",
  "thinspo",
  "self harm",
  "kill yourself",
  "kys",
];

/** Letters people swap for digits and symbols to slip a word past a filter. */
const LEET: Record<string, string> = { "0": "o", "1": "i", "3": "e", "4": "a", "5": "s", "7": "t", "@": "a", $: "s", "!": "i" };

/** The words of `text`, lower case and without accents, once as written and once with look-alikes undone. */
function words(text: string): string[][] {
  const plain = text
    .normalize("NFKD")
    .replace(/\p{M}/gu, "")
    .toLowerCase();
  const split = (s: string) => s.split(/[^a-z0-9]+/).filter(Boolean);
  const unleet = [...plain].map((c) => LEET[c] ?? c).join("");
  return [split(plain), split(unleet)];
}

function hits(terms: string[], text: string): string[] {
  const found = new Set<string>();
  for (const list of words(text)) {
    const joined = ` ${list.join(" ")} `;
    const squashed = list.join("");
    for (const term of terms) {
      // A multi-word term also matches written as one word ("selfharm").
      if (joined.includes(` ${term} `) || (term.includes(" ") && squashed.includes(term.replace(/ /g, "")))) found.add(term);
    }
  }
  return [...found];
}

let profanity: RegExpMatcher | null = null;

/**
 * The flags for the words of a pack: `high` for the blocklist (and the maintainer's own words in
 * EXTRA_BLOCKLIST), `normal` for general profanity, which the obscenity list catches with its
 * spelling tricks undone.
 */
export function textFlags(fields: { label: string; text: string }[], extra = ""): Flag[] {
  profanity ??= new RegExpMatcher({ ...englishDataset.build(), ...englishRecommendedTransformers });
  const terms = [
    ...BLOCKLIST,
    ...extra
      .split(",")
      .map((t) => t.trim().toLowerCase().replace(/[^a-z0-9 ]+/g, " ").replace(/\s+/g, " ").trim())
      .filter(Boolean),
  ];
  const flags: Flag[] = [];
  for (const { label, text } of fields) {
    if (!text) continue;
    for (const term of hits(terms, text)) {
      flags.push({ code: "text:blocklist", severity: "high", detail: `"${term}" in ${label}` });
    }
    if (profanity.hasMatch(text)) {
      flags.push({ code: "text:profanity", severity: "normal", detail: `profanity in ${label}` });
    }
  }
  return flags;
}
