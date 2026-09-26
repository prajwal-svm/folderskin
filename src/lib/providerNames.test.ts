/// <reference types="node" />
import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";

vi.mock("../i18n", () => ({ t: (key: string) => (key === "ai.local.name" ? "Modèle local" : key) }));

const { LOCAL_MODEL, madeWith, providerName } = await import("./providerNames");

describe("the Local Model's name", () => {
  it("is the one the Rust side gives it", () => {
    const rust = readFileSync(new URL("../../src-tauri/src/ai/local.rs", import.meta.url), "utf8");
    expect(rust).toContain(`label: "${LOCAL_MODEL}".into()`);
  });

  it("is said in the language on show, and a brand's name stays as it is", () => {
    expect(providerName("Local Model")).toBe("Modèle local");
    expect(providerName("OpenAI")).toBe("OpenAI");
  });

  it("is said in the language on show where it says what made a picture", () => {
    expect(madeWith("Local Model · FLUX.2 klein 4B")).toBe("Modèle local · FLUX.2 klein 4B");
    expect(madeWith("OpenAI · GPT Image 2.5 Flare")).toBe("OpenAI · GPT Image 2.5 Flare");
    expect(madeWith("Local Model")).toBe("Modèle local");
  });
});

describe("a model no longer offered", () => {
  it("moves on to the one that took its place, and a model on offer stays as it is", async () => {
    const { currentModel } = await import("./providerNames");
    const model = (id: string, label: string) => ({ id, label, native_alpha: false, accepts_reference: true, max_references: 16, sizes: ["1024x1024"], price_hint: "" });
    const openai = {
      id: "openai",
      label: "OpenAI",
      models: [model("gpt-image-2.5-flare", "GPT Image 2.5 Flare"), model("gpt-image-2", "GPT Image 2")],
      keys_url: "",
      docs_url: "",
      key_hint: "",
      has_key: true,
      retired: [{ id: "gpt-image-1", successor: "gpt-image-2" }],
    };
    expect(currentModel(openai, "gpt-image-1")).toBe("gpt-image-2");
    expect(currentModel(openai, "gpt-image-2.5-flare")).toBe("gpt-image-2.5-flare");
    expect(currentModel(openai, "nope")).toBe("nope");
    expect(currentModel(undefined, "gpt-image-1")).toBe("gpt-image-1");
  });

  it("is listed by the Rust side, for every model the catalogue took out", () => {
    const rust = readFileSync(new URL("../../src-tauri/src/ai.rs", import.meta.url), "utf8");
    expect(rust).toContain("folderskin_ai::catalogue::RETIRED");
  });
});
