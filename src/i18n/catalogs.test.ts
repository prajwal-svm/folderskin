/// <reference types="node" />
import { readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { flatten } from "./core";

const root = new URL("../../", import.meta.url);
const read = (path: string) => readFileSync(new URL(path, root), "utf8");

/** The app's own code by its path under src/: every .ts and .tsx file but the tests. */
function sources(): Map<string, string> {
  const out = new Map<string, string>();
  for (const name of readdirSync(new URL("src/", root), { recursive: true, encoding: "utf8" })) {
    const path = name.split("\\").join("/");
    if (!/\.tsx?$/.test(path) || path.endsWith(".test.ts") || path.endsWith(".d.ts")) continue;
    out.set(path, read(`src/${path}`));
  }
  return out;
}

const namespaces = readdirSync(new URL("src/locales/en/", root))
  .filter((f) => f.endsWith(".json"))
  .map((f) => f.slice(0, -".json".length))
  .sort();

const english = Object.fromEntries(namespaces.map((ns) => [ns, JSON.parse(read(`src/locales/en/${ns}.json`))]));

describe("the English catalogs", () => {
  it("each come with the app, or with the composer", () => {
    const loaders = `${read("src/i18n/index.ts")}\n${read("src/i18n/composer.ts")}`;
    const imported = [...loaders.matchAll(/from "\.\.\/locales\/en\/([\w-]+)\.json"/g)].map((m) => m[1]).sort();
    expect(imported).toEqual(namespaces);
  });

  it("hold no message the app doesn't show", () => {
    // A message is used when the code names it in full, or builds its key from its group, as
    // `common.size.${unit}` does. The engine's sentences are found by their words (lib/sentences.ts).
    const code = [...sources().values()].join("\n");
    const unused: string[] = [];
    for (const ns of namespaces) {
      if (ns === "native") continue;
      const keys = new Set(Object.keys(flatten(english[ns], ns)).map((k) => k.replace(/_(zero|one|two|few|many|other)$/, "")));
      for (const key of keys) {
        if (code.includes(`"${key}"`) || code.includes(`\`${key}\``)) continue;
        const parts = key.split(".");
        const built = parts.slice(1).some((_, i) => code.includes(`\`${parts.slice(0, i + 1).join(".")}.\${`));
        if (!built) unused.push(key);
      }
    }
    expect(unused).toEqual([]);
  });
});

describe("the composer's words", () => {
  it("come with every file that uses them", () => {
    // They're in the composer's own chunk (i18n/composer.ts), so a file that shows one brings them.
    const missing: string[] = [];
    for (const [path, text] of sources()) {
      // A key, not a file's name like `composer.rs`.
      if (path.startsWith("i18n/") || !/["'`]composer\.(?!(?:rs|ts|tsx|json|css)\b)[a-zA-Z]/.test(text)) continue;
      if (!/import\s+["'](?:\.\.?\/)+i18n\/composer["']/.test(text)) missing.push(path);
    }
    expect(missing).toEqual([]);
  });
});

describe("the menu bar's words", () => {
  it("are the ones the webview sends and the Rust side builds the menu from", () => {
    const words = Object.keys(english.menu).sort();
    const sent = /const MENU = \[([^\]]*)\]/.exec(read("src/state/language.ts"))?.[1] ?? "";
    expect([...sent.matchAll(/"(\w+)"/g)].map((m) => m[1]).sort()).toEqual(words);
    const struct = /pub struct MenuWords \{([^}]*)\}/.exec(read("src-tauri/src/language.rs"))?.[1] ?? "";
    const fields = [...struct.matchAll(/pub (\w+): String/g)].map((m) => m[1].replace(/_(\w)/g, (_, c: string) => c.toUpperCase()));
    expect(fields.sort()).toEqual(words);
  });
});
