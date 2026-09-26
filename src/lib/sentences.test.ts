/// <reference types="node" />
import { readdirSync, readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { flatten, PLACEHOLDER } from "../i18n/core";
import native from "../locales/en/native.json";

const english = flatten(native, "native");

/**
 * Every string literal in the Rust the app is built from, outside comments, with a line that ends
 * in `\` joined to the next as Rust joins it. Format arguments (`{}`, `{name}`, `{:.0}`) become `{}`.
 */
function rustStrings(): string[] {
  const root = new URL("../../", import.meta.url);
  const dirs = ["src-tauri/src", "crates/folderskin-core/src", "crates/folderskin-ai/src", "crates/folderskin-local/src", "crates/folderskin-share/src", "crates/folderskin-catalog/src"];
  const out: string[] = [];
  for (const dir of dirs) {
    for (const name of readdirSync(new URL(`${dir}/`, root), { recursive: true, encoding: "utf8" })) {
      if (!name.endsWith(".rs")) continue;
      const src = readFileSync(new URL(`${dir}/${name.split("\\").join("/")}`, root), "utf8");
      for (let i = 0; i < src.length; i++) {
        if (src[i] === "/" && src[i + 1] === "/") {
          i = src.indexOf("\n", i);
          if (i < 0) break;
          continue;
        }
        if (src[i] === "'") {
          // A character ('"', '\'', '\n') or a lifetime ('a): neither holds a string.
          if (src[i + 1] === "\\") i = src.indexOf("'", i + 3);
          else if (src[i + 2] === "'") i += 2;
          continue;
        }
        // A raw string (r"…", r#"…"#): no escapes, so a backslash before its end is only a backslash.
        if (src[i] === "r" && /["#]/.test(src[i + 1]) && !/\w/.test(src[i - 1] ?? "")) {
          let hashes = "";
          let j = i + 1;
          while (src[j] === "#") hashes += src[j++];
          if (src[j] === '"') {
            const end = src.indexOf(`"${hashes}`, j + 1);
            out.push(src.slice(j + 1, end).replace(/\{[^{}]*\}/g, "{}"));
            i = end + hashes.length;
            continue;
          }
        }
        if (src[i] !== '"') continue;
        let lit = "";
        let j = i + 1;
        for (; j < src.length && src[j] !== '"'; j++) {
          if (src[j] === "\\") {
            if (src[j + 1] === "\n") {
              j += 2;
              while (/\s/.test(src[j])) j++;
              j--;
              continue;
            }
            lit += src[j + 1] === '"' ? '"' : src[j] + src[j + 1];
            j++;
            continue;
          }
          lit += src[j];
        }
        out.push(lit.replace(/\{[^{}]*\}/g, "{}"));
        i = j;
      }
    }
  }
  return out;
}

describe("the sentences the Rust side writes", () => {
  const strings = rustStrings().map((s) => s.toLowerCase());

  it("are all still written, word for word, by the Rust side", () => {
    const missing: string[] = [];
    for (const [key, value] of Object.entries(english)) {
      // The system's own words for a failed file operation, which Rust's std passes on.
      if (key.startsWith("native.os.")) continue;
      const pieces = value
        .replace(/\.$/, "")
        .split(PLACEHOLDER)
        .filter((_, i) => i % 2 === 0)
        .map((p) => p.toLowerCase())
        .filter((p) => p.trim().length > 1);
      if (!pieces.every((p) => strings.some((s) => s.includes(p)))) missing.push(`${key}: ${value}`);
    }
    expect(missing).toEqual([]);
  });
});

describe("explain", async () => {
  const actual = await vi.importActual<typeof import("../i18n")>("../i18n");
  const said: { key: string; vars?: Record<string, unknown> }[] = [];

  it("gives back the English as it was", async () => {
    const { explain } = await import("./sentences");
    for (const sentence of [
      "you don't have permission to change it",
      "this is your home folder. Pick a folder inside it instead",
      "Add your OpenAI API key first.",
      "add your OpenAI API key first",
      "that's 60 skins, and a pack holds at most 50",
      "couldn't reach packs.folderskin.app. Check your connection and try again",
      "b.png: it isn't on github.com any more. Try Refresh",
      "Permission denied (os error 13)",
      "Sending your idea to Google Gemini",
      "~$0.04 / image",
      "something no one wrote down",
    ]) {
      expect(explain(sentence)).toBe(sentence);
    }
    expect(actual.getLocale()).toBe("en");
  });

  it("says a known sentence again by its key, with its parts, and the parts that are sentences too", async () => {
    vi.resetModules();
    vi.doMock("../i18n", () => ({
      ...actual,
      t: (key: string, vars?: Record<string, unknown>) => {
        said.push({ key, vars });
        return `<${key}>`;
      },
    }));
    const { explain } = await import("./sentences");
    expect(explain("its disk is full")).toBe("<native.folder.full>");
    expect(explain("that's 60 skins, and a pack holds at most 50")).toBe("<native.community.tooManySkins>");
    expect(said.at(-1)?.vars).toEqual({ count: "60", max: "50" });
    // The reason inside is one of the known sentences too.
    expect(explain("couldn't write the pack: Permission denied (os error 13)")).toBe("<native.community.writeFailed>");
    expect(said.at(-1)?.vars).toEqual({ reason: "<native.os.permission>" });
    // A file's name, then a known sentence.
    expect(explain("b.png: it arrived damaged. Try again")).toBe("b.png: <native.community.damaged>");
    // A provider's failures: a picture it didn't make, its filter, and its own words with the status.
    expect(explain("Google Gemini finished without painting a picture. Try again, or reword the idea.")).toBe("<native.ai.noImage>");
    expect(said.at(-1)?.vars).toEqual({ provider: "Google Gemini" });
    expect(explain("xAI Grok declined that prompt: its filter blocked the picture.")).toBe("<native.ai.refused>");
    expect(said.at(-1)?.vars).toEqual({ provider: "xAI Grok", reason: "<native.ai.filterBlocked>" });
    expect(explain("OpenAI said: Your organization must be verified to use the model. (error 403)")).toBe("<native.ai.said>");
    expect(said.at(-1)?.vars).toEqual({ provider: "OpenAI", reason: "Your organization must be verified to use the model.", status: "403" });
    // Anything else is shown as it came.
    expect(explain("the disk made a noise")).toBe("the disk made a noise");
    vi.doUnmock("../i18n");
  });
});
