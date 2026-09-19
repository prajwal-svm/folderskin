/** Community packs live in the FolderSkin repository on GitHub, under community/. */
export const REPO_URL = "https://github.com/prajwal-svm/folderskin";
/** The pack folders, to look through on GitHub. */
export const PACKS_URL = `${REPO_URL}/tree/main/community/packs`;
/** GitHub's upload page for that folder: drop a pack folder there to propose it. */
export const UPLOAD_URL = `${REPO_URL}/upload/main/community/packs`;
/** The pack contract and how to share one. */
export const PACKS_GUIDE_URL = `${REPO_URL}/blob/main/docs/PACKS.md`;

/** Most skins in one pack; `folderskin_core::pack::MAX_SKINS` is the same. */
export const MAX_PACK_SKINS = 24;

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

/** What sharing remembers between packs: the GitHub user name and the usual licence. */
export type SharingPrefs = { author: string; license: string };
const SHARING_KEY = "folderskin.sharing";

export function loadSharingPrefs(): SharingPrefs {
  try {
    const saved = JSON.parse(localStorage.getItem(SHARING_KEY) ?? "{}") as Partial<SharingPrefs>;
    const license = LICENSES.some((l) => l.id === saved.license) ? (saved.license as string) : LICENSES[0].id;
    return { author: typeof saved.author === "string" ? saved.author : "", license };
  } catch {
    return { author: "", license: LICENSES[0].id };
  }
}

export function saveSharingPrefs(prefs: SharingPrefs): void {
  try {
    localStorage.setItem(SHARING_KEY, JSON.stringify(prefs));
  } catch {
    /* nowhere to keep it: it lasts until the app closes */
  }
}

/** A GitHub user name: letters, digits and single dashes, not at either end, at most 39. */
export function isGithubUser(name: string): boolean {
  return name.length <= 39 && /^[A-Za-z0-9]+(-[A-Za-z0-9]+)*$/.test(name);
}

/** The folder name a pack gets from its name, the way `pack::slug` makes it in Rust. */
export function packSlug(name: string): string {
  let out = "";
  for (const c of name.toLowerCase()) {
    if (/[a-z0-9]/.test(c)) out += c;
    else if (out && !out.endsWith("-")) out += "-";
  }
  const slug = out.replace(/-+$/, "").slice(0, 40).replace(/-+$/, "");
  // Windows keeps these names for devices, and git there can't check them out.
  return /^(con|prn|aux|nul|com\d|lpt\d)$/.test(slug) ? `${slug}-1` : slug;
}
