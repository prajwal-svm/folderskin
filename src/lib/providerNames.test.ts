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
