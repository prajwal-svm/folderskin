import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { chatPrompt, CHIP_STYLES, STYLES, styleTags, SUGGESTIONS, suggestion, surprise } from "./prompts";
import ai from "../locales/en/ai.json";

describe("the ideas the chips fill in", () => {
  it("gives every chip at least two ideas that say what the picture shows, and leave the style to its slot", () => {
    for (const s of CHIP_STYLES) {
      const list = SUGGESTIONS[s.id];
      expect(list.length, s.id).toBeGreaterThanOrEqual(2);
      for (const text of list) {
        expect(text.length, text).toBeGreaterThan(40);
        expect(text.length, text).toBeLessThan(200);
        // The style goes in its own slot, after the idea: an idea that names one would say it twice.
        expect(text).not.toMatch(/\bas an? /i);
        expect(styleTags(text), text).toEqual([]);
      }
    }
    for (const id of Object.keys(SUGGESTIONS)) expect(STYLES.some((s) => s.id === id), id).toBe(true);
  });

  it("never asks for colours the magenta cut-out would remove", () => {
    for (const text of Object.values(SUGGESTIONS).flat()) {
      expect(text.toLowerCase()).not.toMatch(/magenta|fuchsia|hot pink|\bpink\b/);
    }
  });

  it("cycles through a chip's ideas on repeated clicks", () => {
    const [first, second] = SUGGESTIONS.woodblock;
    expect(suggestion("woodblock", 0)).toBe(first);
    expect(suggestion("woodblock", 1)).toBe(second);
    expect(suggestion("woodblock", 2)).toBe(first);
    // A style named by the id it had before still finds its ideas.
    expect(suggestion("ukiyoe", 0)).toBe(first);
    expect(suggestion("nope", 0)).toBe("");
  });

  it("surprises with a real idea and says which style it goes with", () => {
    const pick = surprise(() => 0.99);
    expect(SUGGESTIONS[pick.styleId]).toContain(pick.text);
    expect(pick.text).toBe(suggestion(pick.styleId, pick.index));
    expect(CHIP_STYLES.some((s) => s.id === pick.styleId)).toBe(true);
  });
});

describe("style tags", () => {
  it("find styles in the user's own words, and nothing when there is none", () => {
    expect(styleTags("a fox, as an Ukiyo-e print")).toEqual(["woodblock"]);
    expect(styleTags("a fox in the snow")).toEqual([]);
  });
});

describe("chat prompt", () => {
  it("leaves placeholders when nothing is filled in", () => {
    const p = chatPrompt("", null);
    expect(p).toContain("Scene: DESCRIBE THE SCENE");
    expect(p).toContain("Style: DESCRIBE THE STYLE");
  });

  it("uses the description alone when no style is picked", () => {
    const p = chatPrompt(SUGGESTIONS.clay[0], null);
    expect(p).toContain(`Scene: ${SUGGESTIONS.clay[0]}`);
    expect(p).not.toContain("Style:");
  });

  it("adds the style picked, in the words the models get", () => {
    const p = chatPrompt("a whale over a harbour", "risograph");
    expect(p).toContain("Style: a risograph-printed illustration: two or three flat spot colours");
    expect(chatPrompt("a whale", "riso")).toContain("Style: a risograph-printed illustration");
  });

  it("always asks for the flat magenta background the importer cuts away", () => {
    expect(chatPrompt("x", null)).toContain("pure flat magenta #FF00FF");
  });
});

describe("docs/PROMPTS.md", () => {
  // Line breaks in the guide are only where its lines wrap.
  const flat = (s: string) => s.replace(/\s+/g, " ");
  const guide = flat(readFileSync(new URL("../../docs/PROMPTS.md", import.meta.url), "utf8"));

  it("has the prompt the app fills in, word for word", () => {
    expect(guide).toContain(flat(chatPrompt("", null)));
  });

  it("lists every style the app offers, as the app names and describes it", () => {
    for (const s of STYLES) {
      const name = ai.styles[s.id as keyof typeof ai.styles];
      const said = ai.styleDescriptions[s.id as keyof typeof ai.styleDescriptions];
      expect(guide, s.id).toContain(`**${name}**: ${said.charAt(0).toLowerCase()}${said.slice(1)}`);
    }
  });

  it("has every idea the style buttons fill in, under the style it was written for", () => {
    for (const [id, ideas] of Object.entries(SUGGESTIONS)) {
      const name = ai.styles[id as keyof typeof ai.styles];
      for (const idea of ideas) expect(guide, idea).toContain(`**${name}.** ${idea}`);
    }
  });
});
