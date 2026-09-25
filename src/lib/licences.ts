import { t } from "../i18n";

/**
 * The licences shared skins can use: Creative Commons, or MIT like FolderSkin's own code.
 * `folderskin_core::pack::LICENSES` is the same list. CC0 comes first: most skins are made with
 * AI, and CC0 claims the least over them. What each lets people do is `common.licences.<key>`.
 */
export const LICENSES = [
  { id: "CC0-1.0", label: "CC0", key: "cc0" },
  { id: "CC-BY-4.0", label: "CC BY 4.0", key: "ccBy" },
  { id: "MIT", label: "MIT", key: "mit" },
] as const;

/** A licence as a choice in a list: its name and what it lets people do. */
export const licenceOption = (l: (typeof LICENSES)[number]) => t("common.licences.option", { label: l.label, note: t(`common.licences.${l.key}`) });

export const licenseLabel = (id: string) => LICENSES.find((l) => l.id === id)?.label ?? id;

/** A name a pack can be credited to, shaped like a GitHub user name: letters, digits and single
 *  dashes, not at either end, at most 39. pack.json's `author` has always had that shape. */
export function isGithubUser(name: string): boolean {
  return name.length <= 39 && /^[A-Za-z0-9]+(-[A-Za-z0-9]+)*$/.test(name);
}
