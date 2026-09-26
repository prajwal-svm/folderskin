import { describe, expect, it } from "vitest";
import { findTrigger, moveActive, namesSomeone, narrow, removeTrigger, replaceTrigger, rowsOf, slashMenu, suggestName, takenBy } from "./promptMenu";

describe("finding a trigger", () => {
  it("finds @ and / at the start of the box or of a word, up to the caret", () => {
    expect(findTrigger("@", 1)).toEqual({ kind: "@", query: "", start: 0, end: 1 });
    expect(findTrigger("a fox /wood", 11)).toEqual({ kind: "/", query: "wood", start: 6, end: 11 });
    expect(findTrigger("a fox\n@win", 10)).toEqual({ kind: "@", query: "win", start: 6, end: 10 });
    // Only what's before the caret counts: the caret mid-word still has its trigger.
    expect(findTrigger("a /woodblock fox", 7)).toEqual({ kind: "/", query: "wood", start: 2, end: 7 });
  });

  it("isn't fooled by slashes and at signs inside words", () => {
    expect(findTrigger("and/or", 6)).toBeNull();
    expect(findTrigger("see https://example.com", 23)).toBeNull();
    expect(findTrigger("mail me@example.com", 19)).toBeNull();
    // A space ends it: the menu closes as the next word starts.
    expect(findTrigger("a /wood block", 13)).toBeNull();
    expect(findTrigger("", 0)).toBeNull();
  });
});

describe("taking a trigger out", () => {
  const at = (text: string) => findTrigger(text, text.length)!;

  it("leaves the words around it, spaced, with the caret ready for the next word", () => {
    expect(removeTrigger("a fox /wood", at("a fox /wood"))).toEqual({ text: "a fox ", caret: 6 });
    expect(removeTrigger("@win", at("@win"))).toEqual({ text: "", caret: 0 });
    const mid = findTrigger("a @free fox", 7)!;
    expect(removeTrigger("a @free fox", mid)).toEqual({ text: "a fox", caret: 2 });
  });

  it("puts what's chosen where the trigger was, or makes it the whole box", () => {
    const idea = "a night sky with aurora";
    expect(replaceTrigger("/aur", at("/aur"), idea)).toEqual({ text: idea, caret: idea.length });
    expect(replaceTrigger("moody, /aur", at("moody, /aur"), idea)).toEqual({ text: `moody, ${idea}`, caret: 7 + idea.length });
    const mid = findTrigger("/aur over hills", 4)!;
    expect(replaceTrigger("/aur over hills", mid, idea)).toEqual({ text: `${idea} over hills`, caret: idea.length + 1 });
  });
});

describe("the menus", () => {
  const styleList = [
    { id: "woodblock", name: "Woodblock print", note: "Carved keylines and flat colour", current: false },
    { id: "oil", name: "Oil painting", note: "Rich paint and dramatic light", current: true },
    { id: "clay", name: "Clay", note: "Soft handmade clay", current: false },
  ];
  const styles = [
    { id: "print", title: "Print", items: [styleList[0]] },
    { id: "paint", title: "Painting and drawing", items: [styleList[1]] },
    { id: "craft", title: "Materials and craft", items: [styleList[2]] },
  ];
  const ideas = [
    { id: "aurora", name: "Aurora", note: "a night sky with aurora" },
    { id: "wave", name: "Wave", note: "a woodblock ocean wave" },
  ];
  const prompts = [{ id: "p1", name: "Moody koi", note: "a koi pond at night", text: "a koi pond at night", look: { kind: "style" as const, id: "woodblock" } }];
  const titles = { prompts: "Your prompts", ideas: "Ideas" };
  const save = { name: "Save as a prompt", note: "Keeps it" };

  it("narrows to what's typed, best first, ignoring case and accents", () => {
    expect(narrow(styleList, "WOOD").map((s) => s.id)).toEqual(["woodblock"]);
    expect(narrow(styleList, "paint").map((s) => s.id)).toEqual(["oil"]);
    // A name that starts with it comes before one that only says it.
    expect(narrow([...ideas, { id: "wood", name: "Woodland", note: "" }], "wood").map((i) => i.id)).toEqual(["wood", "wave"]);
    expect(narrow([{ id: "x", name: "Crème brûlée", note: "" }], "creme").map((i) => i.id)).toEqual(["x"]);
    expect(narrow(styleList, "").map((s) => s.id)).toEqual(["woodblock", "oil", "clay"]);
    expect(narrow(styleList, "zzz")).toEqual([]);
  });

  it("offers saving first, then your prompts, the styles under their headings and the ideas", () => {
    const groups = slashMenu({ query: "", prompts, styles, ideas, canSave: true, titles, save });
    expect(groups.map((g) => g.id)).toEqual(["save", "prompts", "styles-print", "styles-paint", "styles-craft", "ideas"]);
    expect(groups.map((g) => g.title)).toEqual(["", "Your prompts", "Print", "Painting and drawing", "Materials and craft", "Ideas"]);
    expect(rowsOf(groups).map((r) => `${r.kind}:${r.id}`)).toEqual(["save:save", "prompt:p1", "style:woodblock", "style:oil", "style:clay", "idea:aurora", "idea:wave"]);
    // A saved prompt brings its look with it.
    expect(rowsOf(groups)[1]).toMatchObject({ kind: "prompt", look: { kind: "style", id: "woodblock" } });
    // Once something is typed, the best match comes first and saving goes last.
    const typed = slashMenu({ query: "o", prompts, styles, ideas, canSave: true, titles, save });
    expect(typed.at(-1)?.id).toBe("save");
    expect(typed[0].id).not.toBe("save");
    // A heading with nothing that matches under it goes.
    const clay = slashMenu({ query: "clay", prompts, styles, ideas, canSave: false, titles, save });
    expect(clay.map((g) => g.id)).toEqual(["styles-craft"]);
    // A prompt is found by its words as well as its name.
    const koi = slashMenu({ query: "pond", prompts, styles, ideas, canSave: false, titles, save });
    expect(rowsOf(koi).map((r) => r.id)).toEqual(["p1"]);
    // Nothing to save, no row for it, and groups with nothing in them are left out.
    const none = slashMenu({ query: "zzz", prompts, styles, ideas, canSave: false, titles, save });
    expect(none).toEqual([]);
    const onlySave = slashMenu({ query: "zzz", prompts, styles, ideas, canSave: true, titles, save });
    expect(rowsOf(onlySave).map((r) => r.kind)).toEqual(["save"]);
  });

  it("puts the groups whose names match what's typed before those whose words only mention it", () => {
    const lit = [{ id: "print", title: "Print", items: [{ id: "linocut", name: "Linocut", note: "Bold light and one ink", current: false }] }];
    const lighthouse = [{ id: "lighthouse", name: "Lighthouse", note: "a lighthouse on a rocky point" }];
    const groups = slashMenu({ query: "light", prompts: [], styles: lit, ideas: lighthouse, canSave: false, titles, save });
    expect(groups.map((g) => g.id)).toEqual(["ideas", "styles-print"]);
    // Nothing typed, the usual order.
    expect(slashMenu({ query: "", prompts: [], styles: lit, ideas: lighthouse, canSave: false, titles, save }).map((g) => g.id)).toEqual(["styles-print", "ideas"]);
  });

  it("moves through the rows and round at either end", () => {
    expect(moveActive(0, 1, 3)).toBe(1);
    expect(moveActive(2, 1, 3)).toBe(0);
    expect(moveActive(0, -1, 3)).toBe(2);
    expect(moveActive(0, 1, 0)).toBe(0);
  });
});

describe("saving a prompt", () => {
  it("notices a style named after someone, as the app's own rule does", () => {
    expect(namesSomeone("a forest spirit in the style of Studio Ghibli")).toBe("Studio Ghibli");
    expect(namesSomeone("a portrait by Van Gogh.")).toBe("Van Gogh");
    expect(namesSomeone("lit by candlelight")).toBeNull();
    expect(namesSomeone("in the style of the old masters")).toBeNull();
    expect(namesSomeone("a fox by A")).toBeNull();
  });

  it("suggests its first words as its name, cut at a word", () => {
    expect(suggestName("a koi pond at night, lit by lanterns and fireflies over the water")).toBe("a koi pond at night, lit by");
    expect(suggestName("  a fox.  ")).toBe("a fox");
    expect(suggestName("Supercalifragilisticexpialidocious-and-more-words-joined", 20)).toBe("Supercalifragilistic");
  });

  it("knows a name that's taken, whatever its capitals and spaces", () => {
    const saved = [{ name: "Moody koi" }, { name: "Fox" }];
    expect(takenBy(saved, "  moody   KOI ")).toBe(saved[0]);
    expect(takenBy(saved, "Foxes")).toBeUndefined();
    expect(takenBy(saved, "  ")).toBeUndefined();
  });
});
