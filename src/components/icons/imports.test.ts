/// <reference types="node" />
import { readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

/**
 * Every source file's text, with its path from the repository root. Tests are left out so their
 * patterns do not count. Read from disk rather than imported with `?raw`: a raw import counts as
 * running the file, so the coverage report called every component tested.
 */
const root = new URL("../../../", import.meta.url);
const files = readdirSync(new URL("src/", root), { recursive: true, encoding: "utf8" })
  .filter((name) => /\.tsx?$/.test(name) && !name.endsWith(".test.ts"))
  .map((name) => {
    const path = `/src/${name.split("\\").join("/")}`;
    return { path, text: readFileSync(new URL(path.slice(1), root), "utf8") };
  });

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
