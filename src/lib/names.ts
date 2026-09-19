/** Longest name a skin can have, in characters. The Rust store cuts to the same length. */
export const MAX_NAME_CHARS = 60;

/**
 * A typed name made fit to show, the way the store saves it: runs of white space become one
 * space, control characters go, and it is trimmed and cut to `MAX_NAME_CHARS`. Empty when
 * nothing is left.
 */
export function cleanName(name: string): string {
  const oneLine = name.split(/\s+/u).filter(Boolean).join(" ");
  return Array.from(oneLine.replace(/\p{Cc}/gu, ""))
    .slice(0, MAX_NAME_CHARS)
    .join("")
    .trimEnd();
}

/** Longest a name gets inside a sentence or on a button before it's cut short with "…". */
export const CLIP_CHARS = 32;

/**
 * A name cut short to sit inside a sentence, a toast or a button: at most `max` characters,
 * ending in "…" when it was cut, after a whole word when one ends near there. Where a name fills
 * its own line, CSS cuts it to the space there is instead; this is for the names CSS can't reach,
 * in the middle of other words.
 */
export function clip(name: string, max = CLIP_CHARS): string {
  const chars = Array.from(name);
  if (chars.length <= max) return name;
  let cut = chars.slice(0, max - 1).join("");
  const space = cut.lastIndexOf(" ");
  if (chars[max - 1] !== " " && space >= max * 0.6) cut = cut.slice(0, space);
  return `${cut.replace(/[\s,.;:!?–—-]+$/u, "")}…`;
}

/** A sentence that trails off, "…" and all, without a second one after a name `clip` cut short. */
export const trailOff = (text: string) => (text.endsWith("…") ? text : `${text}…`);
