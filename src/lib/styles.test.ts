import { describe, expect, it } from "vitest";
import en from "../locales/en/ai.json";
import table from "../../crates/folderskin-ai/src/styles.json";
import { lookName, sameLook, STYLE_GROUPS, STYLES, styleById, styleDescription, styleGroupName, styleName, styleTags } from "./styles";
import { CHIP_STYLES, SUGGESTIONS } from "./prompts";

describe("the style table", () => {
  it("is the one the prompts are made from, read whole", () => {
    expect(STYLES.map((s) => s.id)).toEqual(table.styles.map((s) => s.id));
    expect(STYLES).toHaveLength(30);
    expect(new Set(STYLES.map((s) => s.id)).size).toBe(STYLES.length);
    expect(styleById("oil")?.fragment).toMatch(/^a classical oil painting: /);
    expect(styleById("nope")).toBeUndefined();
    expect(styleById(null)).toBeUndefined();
  });

  it("finds a style by the id it had before, so an older chat still names it", () => {
    expect(styleById("ukiyoe")?.id).toBe("woodblock");
    expect(styleById("travel")?.id).toBe("screenprint");
    expect(styleById("riso")?.id).toBe("risograph");
    expect(styleName("nouveau")).toBe("Art nouveau");
  });

  it("names, describes and groups every style in English as the table does, for the other languages to translate", () => {
    const names = en.styles as Record<string, string>;
    const descriptions = en.styleDescriptions as Record<string, string>;
    const groups = en.styleGroups as Record<string, string>;
    for (const s of STYLES) {
      expect(names[s.id], s.id).toBe(s.name);
      expect(descriptions[s.id], s.id).toBe(s.description);
      expect(styleName(s.id)).toBe(s.name);
      expect(styleDescription(s.id)).toBe(s.description);
      expect(STYLE_GROUPS.some((g) => g.id === s.group), s.id).toBe(true);
    }
    for (const g of STYLE_GROUPS) {
      expect(groups[g.id], g.id).toBe(g.name);
      expect(styleGroupName(g.id)).toBe(g.name);
    }
    // And nothing in English names a style or a group the table doesn't have.
    expect(Object.keys(names).sort()).toEqual(STYLES.map((s) => s.id).sort());
    expect(Object.keys(descriptions).sort()).toEqual(STYLES.map((s) => s.id).sort());
    expect(Object.keys(groups).sort()).toEqual(STYLE_GROUPS.map((g) => g.id).sort());
  });

  it("finds each style in its own words, so a picture made in it is tagged with it", () => {
    for (const s of STYLES) expect(styleTags(s.fragment), s.id).toContain(s.tag);
  });

  it("offers a chip for every style with ideas to fill in", () => {
    expect(CHIP_STYLES.map((s) => s.id)).toEqual(Object.keys(SUGGESTIONS));
    for (const s of CHIP_STYLES) expect(SUGGESTIONS[s.id]?.length, s.id).toBeGreaterThan(0);
  });
});

describe("a look", () => {
  it("is named by its style in the language on show, or by the prompt it was saved with", () => {
    expect(lookName({ kind: "style", id: "woodblock" })).toBe("Woodblock print");
    expect(lookName({ kind: "skill", id: "night-prints-7k2q", name: "Night prints" })).toBe("Night prints");
  });

  it("is the same look whatever id the style was named by", () => {
    expect(sameLook({ kind: "style", id: "ukiyoe" }, { kind: "style", id: "woodblock" })).toBe(true);
    expect(sameLook({ kind: "style", id: "oil" }, { kind: "style", id: "woodblock" })).toBe(false);
    expect(sameLook({ kind: "skill", id: "a", name: "A" }, { kind: "skill", id: "a", name: "Renamed" })).toBe(true);
    expect(sameLook({ kind: "skill", id: "oil", name: "Oil" }, { kind: "style", id: "oil" })).toBe(false);
    expect(sameLook(null, { kind: "style", id: "oil" })).toBe(false);
  });
});
