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

/** Longest a name gets inside a sentence or on a button before it's cut short. */
export const CLIP_CHARS = 32;

/**
 * A name cut short to sit inside a sentence, a toast or a button: at most `max` characters, after
 * a whole word when one ends near there, and with no "…" (the app's words never trail off in
 * dots). Where a name fills its own line, CSS cuts it to the space there is instead; this is for
 * the names CSS can't reach, in the middle of other words.
 */
export function clip(name: string, max = CLIP_CHARS): string {
  const chars = Array.from(name);
  if (chars.length <= max) return name;
  let cut = chars.slice(0, max).join("");
  const space = cut.lastIndexOf(" ");
  if (chars[max] !== " " && space >= max * 0.6) cut = cut.slice(0, space);
  return cut.replace(/[\s,.;:!?–—-]+$/u, "");
}
