import { describe, expect, it } from "vitest";
import { chatPrompt, STYLES, SUGGESTIONS, suggestion, surprise } from "./prompts";

describe("style briefs", () => {
  it("gives every style at least two briefs that name a subject and a style", () => {
    for (const s of STYLES) {
      const list = SUGGESTIONS[s.id];
      expect(list, s.id).toBeDefined();
      expect(list.length, s.id).toBeGreaterThanOrEqual(2);
      for (const text of list) {
        expect(text.length, text).toBeGreaterThan(60);
        expect(text.length, text).toBeLessThan(240);
        expect(text).toMatch(/\bas an? /i); // "…, as a woodblock print"
      }
    }
  });

  it("never asks for colours the magenta cut-out would remove", () => {
    for (const text of Object.values(SUGGESTIONS).flat()) {
      expect(text.toLowerCase()).not.toMatch(/magenta|fuchsia|hot pink|\bpink\b/);
    }
  });

  it("cycles through a style's briefs on repeated clicks", () => {
    const [first, second] = SUGGESTIONS.ukiyoe;
    expect(suggestion("ukiyoe", 0)).toBe(first);
    expect(suggestion("ukiyoe", 1)).toBe(second);
    expect(suggestion("ukiyoe", 2)).toBe(first);
    expect(suggestion("nope", 0)).toBe("");
  });

  it("surprises with a real brief and says which style it came from", () => {
    const pick = surprise(() => 0.99);
    expect(SUGGESTIONS[pick.styleId]).toContain(pick.text);
    expect(pick.text).toBe(suggestion(pick.styleId, pick.index));
  });
});

describe("chat prompt", () => {
  it("leaves placeholders when nothing is filled in", () => {
    const p = chatPrompt("", null);
    expect(p).toContain("Scene: DESCRIBE THE SCENE");
    expect(p).toContain("Style: DESCRIBE THE STYLE");
  });

  it("uses the description alone when it already carries its style", () => {
    const p = chatPrompt(SUGGESTIONS.clay[0], null);
    expect(p).toContain(`Scene: ${SUGGESTIONS.clay[0]}`);
    expect(p).not.toContain("Style:");
  });

  it("adds the style picked in the helper", () => {
    const p = chatPrompt("a whale over a harbour", "riso");
    expect(p).toContain("Style: risograph print in three inks");
  });

  it("always asks for the flat magenta background the importer cuts away", () => {
    expect(chatPrompt("x", null)).toContain("pure flat magenta #FF00FF");
  });
});
