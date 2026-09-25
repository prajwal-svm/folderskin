/**
 * The Local Model's name in the language on show. The Rust side names the AI providers itself, and
 * those names are brands that stay as they are (OpenAI, Recraft), but "Local Model" is words. It's
 * said again here wherever it shows: as a provider, and in "Local Model · FLUX.2 klein 4B", which
 * says what made a skin or a picture in the chat. What's saved stays in English, so a skin made
 * before a change of language reads right after it.
 */
import { t } from "../i18n";

/** What the Rust side calls the Local Model (src-tauri/src/ai/local.rs). */
export const LOCAL_MODEL = "Local Model";

/** A provider's name as the Rust side gives it, the Local Model's in the language on show. */
export function providerName(label: string): string {
  return label === LOCAL_MODEL ? t("ai.local.name") : label;
}

/** What made a picture, "Local Model · FLUX.2 klein 4B" or "OpenAI · GPT Image", in the language on show. */
export function madeWith(text: string): string {
  const [provider, ...rest] = text.split(" · ");
  return [providerName(provider), ...rest].join(" · ");
}
