/**
 * The built-in styles, from the one table the prompts are made from
 * (crates/folderskin-ai/src/styles.json, read by folderskin_ai::styles as well). A style's words
 * go to the model in the style slot of its prompt, in English whatever language the app is in;
 * its name, its description and its group's heading are shown in the language on show
 * (`ai.styles.<id>`, `ai.styleDescriptions.<id>`, `ai.styleGroups.<id>`), and styles.test.ts
 * keeps the English of those in step with the table.
 */
import table from "../../crates/folderskin-ai/src/styles.json";
import { t, type MessageKey } from "../i18n";

export type Style = {
  id: string;
  /** The ids it had before, which a chat saved then still names it by. */
  aliases: string[];
  /** The heading the "/" menu lists it under. */
  group: string;
  /** Its name in English, as en/ai.json says it. */
  name: string;
  /** One line on what it looks like, in English. */
  description: string;
  /** The words that paint it, read after "as": "a koi pond, as a traditional woodblock-printed illustration: …". */
  fragment: string;
  /** What a picture made in it is tagged with. */
  tag: string;
  /** How someone's own words give it away. */
  words: RegExp;
};

export const STYLES: Style[] = table.styles.map((s) => ({
  id: s.id,
  aliases: "aliases" in s ? (s.aliases as string[]) : [],
  group: s.group,
  name: s.name,
  description: s.description,
  fragment: s.fragment,
  tag: s.tag,
  words: new RegExp(s.words, "i"),
}));

/** The headings, in the order the menu lists them. */
export const STYLE_GROUPS: { id: string; name: string }[] = table.groups;

/** The style called `id`, or that was called `id` before. */
export function styleById(id: string | null | undefined): Style | undefined {
  if (!id) return undefined;
  return STYLES.find((s) => s.id === id) ?? STYLES.find((s) => s.aliases.includes(id));
}

/** A style's name, in the language on show. */
export const styleName = (id: string) => t(`ai.styles.${styleById(id)?.id ?? id}` as MessageKey);

/** What a style looks like, in a line, in the language on show. */
export const styleDescription = (id: string) => t(`ai.styleDescriptions.${styleById(id)?.id ?? id}` as MessageKey);

/** A group's heading, in the language on show. */
export const styleGroupName = (id: string) => t(`ai.styleGroups.${id}` as MessageKey);

/** Tags for an AI result: the styles its description asks for, such as "airbrush". */
export function styleTags(idea: string): string[] {
  return STYLES.filter((s) => s.words.test(idea)).map((s) => s.tag);
}

/**
 * The look a picture is made in: a built-in style, or one of the user's saved prompts that has a
 * look of its own (its own treatment, light, palette or keep-outs), named as they named it.
 */
export type Look = { kind: "style"; id: string } | { kind: "skill"; id: string; name: string };

/** A look's name, in the language on show for a built-in style. */
export function lookName(look: Look): string {
  return look.kind === "style" ? styleName(look.id) : look.name;
}

/** Whether two looks are the same one. */
export const sameLook = (a: Look | null | undefined, b: Look | null | undefined) =>
  Boolean(a && b && a.kind === b.kind && (a.kind === "style" ? styleById(a.id)?.id === styleById(b.id)?.id : a.id === b.id));
