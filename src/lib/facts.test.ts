import { describe, expect, it } from "vitest";
import { skinFacts } from "./facts";
import type { Skin } from "./tauri";

const base: Skin = { id: "user:1", name: "X", collection: "yours", thumbnail: "", custom: true, tags: [], created_at: Date.UTC(2026, 8, 18, 15, 2) };

describe("skin facts", () => {
  it("say which model made an AI result and from what prompt", () => {
    const facts = new Map(
      skinFacts({ ...base, source: "ai", kind: "folder", made_with: "OpenAI · GPT Image 2.5 Flare", idea: "a chrome cassette" }, "en-GB"),
    );
    expect(facts.get("Made with")).toBe("OpenAI · GPT Image 2.5 Flare");
    expect(facts.get("Prompt")).toBe("a chrome cassette");
    expect(facts.get("Shape")).toBe("Whole folder");
    expect(facts.get("Made")).toMatch(/2026/);
  });

  it("credit the pack and the person who shared a community skin", () => {
    const facts = new Map(
      skinFacts({ ...base, source: "community", kind: "artwork", pack_name: "Colours", author: "prajwal-svm", license: "CC-BY-4.0" }),
    );
    expect(facts.get("Pack")).toBe("Colours");
    expect(facts.get("Shared by")).toBe("@prajwal-svm");
    expect(facts.get("Licence")).toBe("CC BY 4.0");
  });

  it("leave out what isn't known", () => {
    const facts = new Map(skinFacts({ ...base, source: "ai", kind: "artwork", created_at: null }));
    expect(facts.has("Made with")).toBe(false);
    expect(facts.has("Made")).toBe(false);
    expect(facts.get("Shape")).toBe("Art on FolderSkin's folder");
  });
});
