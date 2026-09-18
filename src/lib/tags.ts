import type { Skin } from "./tauri";

/** Tags one skin can carry, and the longest tag. The Rust side (`folderskin_core::pack`) keeps the same limits. */
export const MAX_TAGS = 8;
export const MAX_TAG_CHARS = 24;
/** Tags a shared pack gives all of its skins. */
export const MAX_PACK_TAGS = 5;

/**
 * A tag as FolderSkin keeps it, by the same rules as `clean_tag` in Rust: lower case, letters,
 * digits, spaces and dashes only, single spaces, at most `MAX_TAG_CHARS`. Empty when nothing is left.
 */
export function cleanTag(tag: string): string {
  const kept = tag.replace(/[^\p{Alphabetic}\p{Number}\s-]/gu, "").toLowerCase();
  const words = kept.split(/\s+/u).filter(Boolean).join(" ");
  return Array.from(words)
    .slice(0, MAX_TAG_CHARS)
    .join("")
    .replace(/^[ -]+|[ -]+$/g, "");
}

/** Cleans every tag, drops blanks and repeats, and keeps the first `max`. */
export function cleanTags(tags: Iterable<string>, max = MAX_TAGS): string[] {
  const out: string[] = [];
  for (const tag of tags) {
    if (out.length === max) break;
    const clean = cleanTag(tag);
    if (clean && !out.includes(clean)) out.push(clean);
  }
  return out;
}

/** How a tag reads as a filter: its first letter a capital. */
export const tagLabel = (tag: string) => tag.charAt(0).toUpperCase() + tag.slice(1);

/** Every tag on `items` with how many carry it: most used first, then A to Z. */
export function tagCounts(items: { tags?: string[] }[]): { tag: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const item of items) for (const tag of item.tags ?? []) counts.set(tag, (counts.get(tag) ?? 0) + 1);
  return [...counts]
    .map(([tag, count]) => ({ tag, count }))
    .sort((a, b) => b.count - a.count || a.tag.localeCompare(b.tag));
}

/** Skins the user made: pictures they added and AI results. Community skins are everyone's. */
export const isYours = (skin: Skin) => skin.source === "import" || skin.source === "ai";
