import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * Animations that never stop are paused while the window is behind another (`lib/awake.ts`), and
 * each one has to say so itself with `animation-play-state: var(--loop-state)`.
 *
 * This used to be one rule over `*`, which also froze the animations things arrive with: a tile
 * enters with `rise`, which starts at `opacity: 0`, so a gallery first drawn while the window was
 * not in front stayed blank until it was clicked — the counts said 59 and the shelf was empty.
 * Opting in per animation is what stops that coming back, and this is what stops someone adding
 * an endless animation that keeps running in the background because they didn't know to.
 */
const STYLES = join(__dirname);

function declarations(css: string): { line: string; block: string }[] {
  const out: { line: string; block: string }[] = [];
  const lines = css.split("\n");
  lines.forEach((line, i) => {
    if (!/animation:[^;]*\binfinite\b/.test(line)) return;
    // The rest of the rule this sits in, which is where the opt-in belongs.
    let end = i;
    while (end < lines.length && !lines[end].includes("}")) end += 1;
    out.push({ line: line.trim(), block: lines.slice(i, end + 1).join("\n") });
  });
  return out;
}

describe("animations that never stop", () => {
  const files = readdirSync(STYLES).filter((f) => f.endsWith(".css"));

  it("are found at all, so a passing run means something", () => {
    const all = files.flatMap((f) => declarations(readFileSync(join(STYLES, f), "utf8")));
    expect(all.length).toBeGreaterThan(5);
  });

  for (const file of files) {
    const found = declarations(readFileSync(join(STYLES, file), "utf8"));
    if (found.length === 0) continue;
    it(`${file}: every one pauses with the window`, () => {
      for (const { line, block } of found) {
        expect(block, `${file}: \`${line}\` needs animation-play-state: var(--loop-state)`).toMatch(
          /animation-play-state:\s*var\(--loop-state\)/,
        );
      }
    });
  }

  it("the flag itself is defined, and turns off when the window is not in front", () => {
    const base = readFileSync(join(STYLES, "base.css"), "utf8");
    expect(base).toMatch(/:root\s*\{[^}]*--loop-state:\s*running/);
    expect(base).toMatch(/html\[data-awake="false"\]\s*\{[^}]*--loop-state:\s*paused/);
  });

  it("nothing pauses every animation again", () => {
    for (const file of files) {
      const css = readFileSync(join(STYLES, file), "utf8");
      expect(css, `${file} freezes entry animations too`).not.toMatch(
        /html\[data-awake="false"\]\s*\*/,
      );
    }
  });
});
