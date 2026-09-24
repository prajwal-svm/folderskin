/**
 * A first look at a pack's contact sheets with a Workers AI vision model, before a person sees it.
 * It only ever adds flags: nothing is approved because a model found nothing, and nothing is
 * turned down because it found something. A pack it flags as sexual, involving a child or hateful
 * reaches the maintainer at once rather than in the next digest.
 *
 * The free plan gives 10,000 neurons a day; each sheet is budgeted at AI_NEURONS_PER_SHEET against
 * AI_DAILY_NEURONS, and once the day's budget is spent packs simply wait for a person.
 */
import { b64, dayOf, now } from "./bytes";
import type { Env } from "./env";
import { envNumber, take } from "./quota";
import type { Flag } from "./text";

export const DEFAULT_MODEL = "@cf/meta/llama-4-scout-17b-16e-instruct";

const PROMPT = `You check contact sheets of pictures that people want to share as folder icons in an app for everyone, children included.
Answer with one line of JSON and nothing else.
If every picture is fine, answer {"safe": true}.
Otherwise answer {"safe": false, "categories": [...]} using only these categories:
"sexual" (nudity, sexual or suggestive content), "minor" (a child in a sexual or unsafe context),
"hate" (hate symbols, slurs, demeaning people), "violence" (gore, injury, weapons aimed at people),
"self-harm", "drugs", "text" (words or logos that need a closer look).`;

/** Categories the model may name, and how soon a person should look. */
const CATEGORIES: Record<string, Flag["severity"]> = {
  sexual: "high",
  minor: "high",
  hate: "high",
  violence: "normal",
  "self-harm": "high",
  drugs: "normal",
  text: "normal",
};

/** Reads the model's answer into flags. Anything it didn't answer as asked is itself a flag. */
export function readVerdict(output: unknown, sheet: number): Flag[] {
  const response = output && typeof output === "object" ? (output as { response?: unknown }).response : undefined;
  let verdict: unknown = response;
  if (typeof response === "string") {
    const match = response.match(/\{[\s\S]*\}/);
    try {
      verdict = match ? JSON.parse(match[0]) : undefined;
    } catch {
      verdict = undefined;
    }
  }
  if (!verdict || typeof verdict !== "object") {
    return [{ code: "ai:unclear", severity: "normal", detail: `sheet ${sheet + 1}: the model's answer couldn't be read` }];
  }
  const { safe, categories } = verdict as { safe?: unknown; categories?: unknown };
  if (safe === true) return [];
  const named = Array.isArray(categories) ? categories.filter((c): c is string => typeof c === "string" && c in CATEGORIES) : [];
  if (named.length === 0) return [{ code: "ai:unsafe", severity: "normal", detail: `sheet ${sheet + 1}: unsafe, no category given` }];
  return named.map((c) => ({ code: `ai:${c}`, severity: CATEGORIES[c], detail: `sheet ${sheet + 1}` }));
}

/**
 * The flags for these sheets, and whether the model looked at all of them (it doesn't without the
 * binding, once the day's budget is spent, or when it fails, which is never an error for the author).
 */
export async function triage(env: Env, sheets: Uint8Array[], at = now()): Promise<{ flags: Flag[]; looked: boolean }> {
  if (!env.AI || sheets.length === 0) return { flags: [], looked: false };
  const perSheet = envNumber(env.AI_NEURONS_PER_SHEET, 60);
  const budget = envNumber(env.AI_DAILY_NEURONS, 9000);
  const model = env.TRIAGE_MODEL?.trim() || DEFAULT_MODEL;
  const flags: Flag[] = [];
  for (const [n, sheet] of sheets.entries()) {
    if (!(await take(env, dayOf(at), "global", "neurons", perSheet, budget))) return { flags, looked: false };
    try {
      const output = await env.AI.run(model, {
        messages: [
          {
            role: "user",
            content: [
              { type: "text", text: PROMPT },
              { type: "image_url", image_url: { url: `data:${sheetType(sheet)};base64,${b64(sheet)}` } },
            ],
          },
        ],
        max_tokens: 80,
        temperature: 0,
      });
      flags.push(...readVerdict(output, n));
    } catch {
      console.error("folderskin-community: the triage model didn't answer");
      return { flags, looked: false };
    }
  }
  return { flags, looked: true };
}

function sheetType(bytes: Uint8Array): string {
  if (bytes[0] === 0x89) return "image/png";
  if (bytes[0] === 0x52) return "image/webp";
  return "image/jpeg";
}
