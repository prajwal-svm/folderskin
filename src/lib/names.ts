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
