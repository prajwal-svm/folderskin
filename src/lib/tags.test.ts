import { describe, expect, it } from "vitest";
import { cleanTag, cleanTags, MAX_TAG_CHARS, tagCounts, tagLabel } from "./tags";

describe("tags", () => {
  // The same cases as `tags_are_cleaned_the_same_way_everywhere` in folderskin-core's pack.rs.
  it("are cleaned by the same rules as the Rust side", () => {
    expect(cleanTag("  Art   Nouveau ")).toBe("art nouveau");
    expect(cleanTag("#Anime!")).toBe("anime");
    expect(cleanTag("ukiyo-e")).toBe("ukiyo-e");
    expect(cleanTag("- -")).toBe("");
    expect(cleanTag("桜 Sakura")).toBe("桜 sakura");
    expect(cleanTag("a".repeat(40))).toHaveLength(MAX_TAG_CHARS);
    expect(cleanTags(["Glow", "glow", "", "Night"])).toEqual(["glow", "night"]);
    expect(cleanTags(["Glow", "Night"], 1)).toEqual(["glow"]);
  });

  it("read as filters with a capital first letter", () => {
    expect(tagLabel("art nouveau")).toBe("Art nouveau");
  });

  it("are counted most used first, then A to Z", () => {
    const skins = [{ tags: ["pop", "glow"] }, { tags: ["glow"] }, { tags: ["grain"] }, {}];
    expect(tagCounts(skins)).toEqual([
      { tag: "glow", count: 2 },
      { tag: "grain", count: 1 },
      { tag: "pop", count: 1 },
    ]);
  });
});
