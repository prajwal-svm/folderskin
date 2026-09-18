import { licenseLabel } from "./packs";
import type { Skin } from "./tauri";

/** A date and time the way the viewer's own system writes them, such as "18 Sep 2026, 15:02". */
export function formatWhen(ms: number, locale?: string): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(ms);
}

/**
 * What the skin menu can tell about a skin, as label and value pairs: how it was made (the AI
 * model and prompt, or the pack and who shared it), what shape it is, and when it was added.
 */
export function skinFacts(skin: Skin, locale?: string): [string, string][] {
  const facts: [string, string][] = [];
  const added = skin.created_at ? formatWhen(skin.created_at, locale) : null;
  if (skin.source === "ai") {
    if (skin.made_with) facts.push(["Made with", skin.made_with]);
    if (skin.idea) facts.push(["Prompt", skin.idea]);
    facts.push(["Shape", skin.kind === "folder" ? "Whole folder" : "Art on FolderSkin's folder"]);
    if (added) facts.push(["Made", added]);
  } else if (skin.source === "community") {
    if (skin.pack_name) facts.push(["Pack", skin.pack_name]);
    if (skin.author) facts.push(["Shared by", `@${skin.author}`]);
    if (skin.license) facts.push(["Licence", licenseLabel(skin.license)]);
    facts.push(["Shape", skin.kind === "folder" ? "Finished folder" : "Picture on FolderSkin's folder"]);
    if (added) facts.push(["Added", added]);
  } else if (skin.source === "import") {
    facts.push(["From", "A picture you added"]);
    facts.push(["Shape", skin.kind === "folder" ? "Finished folder, background removed" : "Picture on FolderSkin's folder"]);
    if (added) facts.push(["Added", added]);
  } else {
    facts.push(["From", "Comes with FolderSkin"]);
  }
  return facts;
}
