import { isGithubUser } from "./licences";
import { defaultProfile, loadProfiles, saveProfiles, upsertProfile } from "./profiles";

/** FolderSkin's own repository on GitHub: the source, the issues, the releases and the pack docs. */
export const REPO_URL = "https://github.com/prajwal-svm/folderskin";
/** Community packs live in a repository of their own, under packs/. */
export const COMMUNITY_REPO_URL = "https://github.com/prajwal-svm/folderskin-community";
/** The pack folders, to look through on GitHub. */
export const PACKS_URL = `${COMMUNITY_REPO_URL}/tree/main/packs`;
/** FolderSkin's website, where the guides live. */
export const SITE_URL = "https://folderskin.app";
/** The pack contract and how to share one. */
export const PACKS_GUIDE_URL = `${SITE_URL}/docs/packs/`;

/** What someone agrees to before a pack goes up. The version they agree to is the service's
 *  (`ShareStatus.terms_version`), which it records with the pack. */
export const PACK_TERMS_URL = `${SITE_URL}/docs/pack-terms/`;

/** Most skins in one pack; `folderskin_core::pack::MAX_SKINS` is the same. */
export const MAX_PACK_SKINS = 50;

export { isGithubUser, LICENSES, licenseLabel } from "./licences";

/**
 * Gives the default licence profile (profiles.ts) the name a pack was just saved under, when it
 * credits no one yet. That is all sharing ever changes in a profile: which one a pack uses, and a
 * licence changed for one pack, stay with that pack. Profiles are changed in Settings.
 */
export function creditDefaultProfile(name: string): void {
  const profiles = loadProfiles();
  const p = defaultProfile(profiles);
  if (!p.author && isGithubUser(name)) saveProfiles(upsertProfile(profiles, { ...p, author: name }));
}
