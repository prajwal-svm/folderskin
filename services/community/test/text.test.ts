import { describe, expect, it } from "vitest";
import { cleanTag, handleProblem, isPictureFileName, readManifest, slug, textFlags, trimTrailingSlashes } from "../src/text";

const skin = (file: string, name = "Koi", tags: string[] = []) => ({ file, name, tags });

describe("a pack's words", () => {
  it("follow the pack.json rules folderskin_core::pack sets", () => {
    const ok = readManifest({ name: " Night prints ", tags: ["woodblock", "night"], skins: [skin("koi.png"), skin("fox.jpg", "Fox", ["animals"])] });
    expect(ok).toEqual({
      manifest: { name: "Night prints", tags: ["woodblock", "night"], skins: [skin("koi.png"), skin("fox.jpg", "Fox", ["animals"])] },
    });

    const bad = readManifest({ name: "", tags: ["Woodblock!"], skins: [skin("../koi.png"), skin("a.png", "x".repeat(61))] });
    expect("problems" in bad && bad.problems).toEqual([
      "the name must be 1 to 40 characters",
      'the pack: "Woodblock!" isn\'t a tag as FolderSkin writes them',
      "skin 1: the file name isn't one a pack can use",
      "skin 2: the name must be 1 to 60 characters",
    ]);
  });

  it("need at least one tag and one skin, and no more than fifty skins", () => {
    const none = readManifest({ name: "Empty", tags: [], skins: [] });
    expect("problems" in none && none.problems).toContain("the pack needs at least one tag");
    expect("problems" in none && none.problems).toContain("a pack has 1 to 50 skins");
    const many = readManifest({ name: "Many", tags: ["x"], skins: Array.from({ length: 51 }, (_, i) => skin(`s${i}.png`)) });
    expect("problems" in many && many.problems).toContain("a pack has 1 to 50 skins");
  });

  it("keep file names to ones that are safe in a URL and on every system", () => {
    expect(isPictureFileName("koi-2_final.webp")).toBe(true);
    for (const bad of ["koi.gif", ".koi.png", "con.png", "a/b.png", "a b.png", `${"x".repeat(61)}.png`]) {
      expect(isPictureFileName(bad), bad).toBe(false);
    }
  });

  it("clean tags and make folder names the way the app does", () => {
    expect(cleanTag("  Neon   Nights! ")).toBe("neon nights");
    expect(cleanTag("---")).toBeNull();
    expect(slug("Ukiyo-e Nights!")).toBe("ukiyo-e-nights");
    expect(slug("CON")).toBe("con-1");
  });
});

describe("a handle", () => {
  it("is shaped like a GitHub user name, so pack.json v1 accepts it as an author", () => {
    expect(handleProblem("sunny-otter")).toBeNull();
    expect(handleProblem("ab")).toMatch(/3 to 39/);
    expect(handleProblem("-otter")).toMatch(/3 to 39/);
    expect(handleProblem("two--dashes")).toMatch(/3 to 39/);
    expect(handleProblem("x".repeat(40))).toMatch(/3 to 39/);
  });

  it("can't pass for FolderSkin itself or carry a word the lists catch", () => {
    expect(handleProblem("FolderSkin-team")).toMatch(/kept for FolderSkin/);
    expect(handleProblem("admin")).toMatch(/kept for FolderSkin/);
    expect(handleProblem("nsfw-art")).toMatch(/isn't allowed/);
  });
});

describe("the word checks", () => {
  it("flag the blocklist as urgent, with the spelling tricks undone", () => {
    const flags = textFlags([
      { label: "the pack name", text: "N5FW dreams" },
      { label: "the skin names", text: "Self-harm, sunset" },
    ]);
    expect(flags).toEqual([
      { code: "text:blocklist", severity: "high", detail: '"nsfw" in the pack name' },
      { code: "text:blocklist", severity: "high", detail: '"self harm" in the skin names' },
    ]);
  });

  it("flag general profanity for the maintainer without making it urgent", () => {
    const flags = textFlags([{ label: "the credits", text: "what the fuck" }]);
    expect(flags).toEqual([{ code: "text:profanity", severity: "normal", detail: "profanity in the credits" }]);
  });

  it("take the maintainer's own words from EXTRA_BLOCKLIST", () => {
    expect(textFlags([{ label: "the pack tags", text: "brandname, other" }], " BrandName , ")).toHaveLength(1);
  });

  it("leave ordinary words alone", () => {
    expect(textFlags([{ label: "the pack name", text: "Classic art: Scunthorpe harbour, cumulus clouds" }])).toEqual([]);
  });
});

describe("trailing slashes", () => {
  it("come off the end and nowhere else", () => {
    expect(trimTrailingSlashes("https://community.test///")).toBe("https://community.test");
    expect(trimTrailingSlashes("https://community.test/packs/koi")).toBe("https://community.test/packs/koi");
    expect(trimTrailingSlashes("///")).toBe("");
    expect(trimTrailingSlashes("")).toBe("");
  });

  it("take a moment even when a long run of them isn't at the end", () => {
    const text = `${"/".repeat(200_000)}x`;
    expect(trimTrailingSlashes(text)).toBe(text);
    expect(trimTrailingSlashes(`${text}${"/".repeat(200_000)}`)).toBe(text);
  });
});
