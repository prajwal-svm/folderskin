import { isGithubUser } from "./licences";
import { defaultProfile, loadProfiles, saveProfiles, upsertProfile } from "./profiles";

/** FolderSkin's own repository on GitHub: the source, the issues, the releases and the pack docs. */
export const REPO_URL = "https://github.com/prajwal-svm/folderskin";
// The guides (the pack contract, the pack terms) are on the website in each language:
// `docsUrl("packs")` and `docsUrl("pack-terms")` from src/i18n. The version of the terms someone
// agrees to is the service's (`ShareStatus.terms_version`), which it records with the pack.

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
