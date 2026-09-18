import { describe, expect, it } from "vitest";

/** Every source file's text, keyed by path. Tests are left out so their patterns do not count. */
const sources = import.meta.glob(["/src/**/*.{ts,tsx}", "!/src/**/*.test.ts"], {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const files = Object.entries(sources).map(([path, text]) => ({ path, text }));

describe("Motion stays tree-shaken", () => {
  it("never imports the full `motion` component", () => {
    // `motion.*` carries every feature (drag, layout, gestures). Use `m` from "motion/react-m",
    // which gets only the features the LazyMotion in main.tsx loads.
    const full = /import\s*\{[^}]*\bmotion\b[^}]*\}\s*from\s*["']motion\/react["']/;
    expect(files.filter((f) => full.test(f.text)).map((f) => f.path)).toEqual([]);
  });

  it("never imports framer-motion directly", () => {
    const legacy = /from\s*["']framer-motion/;
    expect(files.filter((f) => legacy.test(f.text)).map((f) => f.path)).toEqual([]);
  });

  it("renders every animated icon with `m`", () => {
    const icons = files.filter((f) => f.path.startsWith("/src/components/icons/") && f.text.includes("<m."));
    expect(icons.length).toBeGreaterThanOrEqual(14);
    for (const icon of icons) expect(icon.text, icon.path).toContain('from "motion/react-m"');
  });
});
