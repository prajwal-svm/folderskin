import type { TurnError } from "../state/chats";

/**
 * What an AI request's failure was, from whatever the app threw: a structured error (a code, a
 * sentence, steps to fix it, a prompt to ask for help with) passes through; a plain sentence is
 * given the code its wording says, so the chat can offer the right next step either way.
 */
export function aiFailure(err: unknown): TurnError {
  if (err && typeof err === "object" && !(err instanceof Error)) {
    const e = err as Partial<TurnError> & { what?: string; why?: string };
    const message = e.message ? sentence(e.message) : [e.what, e.why].filter((p): p is string => typeof p === "string" && p.trim() !== "").map(sentence).join(" ");
    if (typeof e.code === "string" && message) {
      return { code: e.code, message, fix: Array.isArray(e.fix) ? e.fix.filter((f): f is string => typeof f === "string") : undefined, ask: typeof e.ask === "string" ? e.ask : undefined };
    }
  }
  const message = err instanceof Error ? err.message : typeof err === "string" ? err : "Something went wrong.";
  return { code: codeOf(message), message: sentence(message) };
}

const PATTERNS: [RegExp, string][] = [
  [/API key first|no key/i, "missing_key"],
  [/key was rejected|unauthori[sz]ed|invalid.*key/i, "unauthorized"],
  [/rate limit|too many requests/i, "rate_limited"],
  // Before a refusal: "the connection was refused" is the network, not the provider saying no.
  [/couldn't reach|network|connection|offline|dns/i, "network"],
  [/declined|refused|safety|content policy/i, "refused"],
  [/took too long|timed? ?out/i, "timeout"],
  [/stopped|cancel+ed/i, "stopped"],
  [/backdrop|scene instead of a folder/i, "no_backdrop"],
  [/not set up|isn't set up|models? (?:aren't|are not) (?:there|downloaded)/i, "local_not_ready"],
  [/out of memory|vram|oom/i, "out_of_memory"],
];

/** The code a plain error sentence's wording points to. */
export function codeOf(message: string): string {
  return PATTERNS.find(([re]) => re.test(message))?.[1] ?? "failed";
}

/** A message as a sentence: capital first, full stop last. */
function sentence(text: string): string {
  const t = text.trim();
  if (!t) return "Something went wrong.";
  const s = t.charAt(0).toUpperCase() + t.slice(1);
  return /[.!?…)]$/.test(s) ? s : `${s}.`;
}

/** Whether trying again, as it was, can help: not when the key, the words or the setup are what's
 *  wrong, nor on a computer the local runtime has no build for (ai/failure.rs passes that code on). */
export function worthRetrying(code: string): boolean {
  return !["missing_key", "unauthorized", "refused", "local_not_ready", "no_build_for_platform"].includes(code);
}
