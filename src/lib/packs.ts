import { isGithubUser, LICENSES } from "./licences";
import { defaultProfile, loadProfiles, saveProfiles, upsertProfile, type LicenseId } from "./profiles";

/** Community packs live in the FolderSkin repository on GitHub, under community/. */
export const REPO_URL = "https://github.com/prajwal-svm/folderskin";
/** The pack folders, to look through on GitHub. */
export const PACKS_URL = `${REPO_URL}/tree/main/community/packs`;
/** GitHub's upload page for that folder: drop a pack folder there to propose it. */
export const UPLOAD_URL = `${REPO_URL}/upload/main/community/packs`;
/** The pack contract and how to share one. */
export const PACKS_GUIDE_URL = `${REPO_URL}/blob/main/docs/PACKS.md`;

/** What someone agrees to before a pack goes up. */
export const PACK_TERMS_URL = `${REPO_URL}/blob/main/docs/PACK-TERMS.md`;
/** Goes up with the terms, and is recorded in the pull request, so what was agreed is never a
 *  question of which version happened to be on main that day. */
export const PACK_TERMS_VERSION = 1;

/** Most skins in one pack; `folderskin_core::pack::MAX_SKINS` is the same. */
export const MAX_PACK_SKINS = 50;

export { isGithubUser, LICENSES, licenseLabel } from "./licences";

/** What sharing starts from: the author and licence of the default licence profile (profiles.ts). */
export type SharingPrefs = { author: string; license: string };

export function loadSharingPrefs(): SharingPrefs {
  const p = defaultProfile(loadProfiles());
  return { author: p.author, license: p.license };
}

/** Keeps what was just shared with in the default profile, so the next pack starts from it. */
export function saveSharingPrefs(prefs: SharingPrefs): void {
  const profiles = loadProfiles();
  const p = defaultProfile(profiles);
  const license = LICENSES.some((l) => l.id === prefs.license) ? (prefs.license as LicenseId) : p.license;
  const author = isGithubUser(prefs.author) ? prefs.author : p.author;
  saveProfiles(upsertProfile(profiles, { ...p, author, license }));
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
