/**
 * The app's own sentences about what went wrong, in the language on show. The Rust side writes its
 * errors as English sentences (docs/ARCHITECTURE.md, "Errors cross the boundary as plain
 * strings"), and the webview shows them. The ones a person is likely to meet are recognised here
 * by their English and said again from the `errors` catalog; anything else is shown as it came.
 * errors.test.ts checks that every sentence listed here is still one the Rust side writes.
 */
import { t, type MessageKey } from "../i18n";

/** One sentence the Rust side writes, and the catalog message that says it in the language on show. */
type Known = {
  /** The whole English sentence, with a named group for each part that fills a placeholder. */
  match: RegExp;
  key: MessageKey;
  /** Words of the English that must appear in a Rust source file, so a rewording there is noticed. */
  rust: string[];
};

/** The sentences known so far, most specific first. */
export const KNOWN: Known[] = [];

/** `message`, a sentence from the Rust side, in the language on show when it's one of the known ones. */
export function explain(message: string): string {
  const text = message.trim();
  for (const known of KNOWN) {
    const m = known.match.exec(text);
    if (m) return t(known.key, m.groups ? { ...m.groups } : undefined);
  }
  return message;
}
