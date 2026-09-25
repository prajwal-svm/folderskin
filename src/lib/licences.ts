/**
 * The licences shared skins can use: Creative Commons, or MIT like FolderSkin's own code.
 * `folderskin_core::pack::LICENSES` is the same list. CC0 comes first: most skins are made with
 * AI, and CC0 claims the least over them.
 */
export const LICENSES = [
  { id: "CC0-1.0", label: "CC0", note: "Anyone can use them for anything." },
  { id: "CC-BY-4.0", label: "CC BY 4.0", note: "Anyone can use them, and credits you." },
  { id: "MIT", label: "MIT", note: "The same licence as FolderSkin; your name stays with them." },
] as const;

export const licenseLabel = (id: string) => LICENSES.find((l) => l.id === id)?.label ?? id;

/** A name a pack can be credited to, shaped like a GitHub user name: letters, digits and single
 *  dashes, not at either end, at most 39. pack.json's `author` has always had that shape. */
export function isGithubUser(name: string): boolean {
  return name.length <= 39 && /^[A-Za-z0-9]+(-[A-Za-z0-9]+)*$/.test(name);
}
