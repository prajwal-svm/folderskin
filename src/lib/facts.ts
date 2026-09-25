import { licenseLabel } from "./packs";
import type { Skin } from "./tauri";
import { getLocale, INTL_LOCALES, t } from "../i18n";
import { madeWith } from "./providerNames";

/** A date and time the way the language on show writes them, such as "18 Sept 2026, 15:02". */
export function formatWhen(ms: number, locale?: string): string {
  return new Intl.DateTimeFormat(locale ?? INTL_LOCALES[getLocale()], { dateStyle: "medium", timeStyle: "short" }).format(ms);
}

/**
 * What the skin menu can tell about a skin, as label and value pairs: how it was made (the AI
 * model and prompt, or the pack and who shared it), what shape it is, and when it was added.
 */
export function skinFacts(skin: Skin, locale?: string): [string, string][] {
  const facts: [string, string][] = [];
  const added = skin.created_at ? formatWhen(skin.created_at, locale) : null;
  if (skin.source === "ai") {
    if (skin.made_with) facts.push([t("library.facts.madeWith"), madeWith(skin.made_with)]);
    if (skin.idea) facts.push([t("library.facts.prompt"), skin.idea]);
    facts.push([t("library.facts.shape"), skin.kind === "folder" ? t("library.facts.wholeFolder") : t("library.facts.artOnFolder")]);
    if (added) facts.push([t("library.facts.made"), added]);
  } else if (skin.source === "community") {
    if (skin.pack_name) facts.push([t("library.facts.pack"), skin.pack_name]);
    if (skin.author) facts.push([t("library.facts.sharedBy"), `@${skin.author}`]);
    if (skin.license) facts.push([t("library.facts.licence"), licenseLabel(skin.license)]);
    facts.push([t("library.facts.shape"), skin.kind === "folder" ? t("library.facts.finishedFolder") : t("library.facts.pictureOnFolder")]);
    if (added) facts.push([t("library.facts.added"), added]);
  } else if (skin.source === "composer") {
    facts.push([t("library.facts.from"), t("library.facts.designedHere")]);
    facts.push([t("library.facts.shape"), t("library.facts.finishedFolder")]);
    if (added) facts.push([t("library.facts.made"), added]);
  } else if (skin.source === "import") {
    facts.push([t("library.facts.from"), t("library.facts.pictureAdded")]);
    facts.push([t("library.facts.shape"), skin.kind === "folder" ? t("library.facts.finishedCutOut") : t("library.facts.pictureOnFolder")]);
    if (added) facts.push([t("library.facts.added"), added]);
  } else {
    facts.push([t("library.facts.from"), t("library.facts.comesWith")]);
  }
  return facts;
}
