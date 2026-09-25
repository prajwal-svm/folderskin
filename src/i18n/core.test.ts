import { describe, expect, it, vi } from "vitest";
import { flatten, interpolate, placeholders, pluralCategories, pluralCategory, richParts, tags, translate, type Lookup } from "./core";

const english = flatten(
  {
    sidebar: { addPhoto: "Add your photo", settings: "Settings" },
    folder: { count_one: "{{count}} folder", count_other: "{{count}} folders", named: "{{name}} has {{count}} skins" },
  },
  "",
);

/** The spaces `Intl` writes (no-break, narrow no-break) as plain ones, which differ by ICU version. */
const plain = (s: string) => s.replace(/[\u00a0\u202f]/g, " ");

const lookupIn = (locale: string, intl: string, active: Record<string, string>): Lookup => ({ locale, intl, active, fallback: english });

describe("catalogs", () => {
  it("are flat maps of every message by its full key", () => {
    expect(english).toEqual({
      "sidebar.addPhoto": "Add your photo",
      "sidebar.settings": "Settings",
      "folder.count_one": "{{count}} folder",
      "folder.count_other": "{{count}} folders",
      "folder.named": "{{name}} has {{count}} skins",
    });
    expect(flatten({ a: { b: 3, c: ["x"], d: null, e: "yes" } }, "ns")).toEqual({ "ns.a.e": "yes" });
    expect(flatten("not a tree", "ns")).toEqual({});
  });
});

describe("placeholders", () => {
  it("are filled, numbers written the way the language writes them", () => {
    expect(interpolate("{{name}} has {{count}} skins", { name: "Trips", count: 1234 }, "en-GB")).toBe("Trips has 1,234 skins");
    expect(plain(interpolate("{{count}} habillages", { count: 1234 }, "fr"))).toBe("1 234 habillages");
    expect(interpolate("{{count}} carpetas", { count: 12345 }, "es")).toBe("12.345 carpetas");
  });

  it("allow spaces inside the braces, and a missing one shows rather than vanishing", () => {
    expect(interpolate("Hi {{ name }}", { name: "Ada" }, "en-GB")).toBe("Hi Ada");
    expect(interpolate("Hi {{name}} and {{other}}", { name: "Ada" }, "en-GB")).toBe("Hi Ada and {{other}}");
    expect(interpolate("No braces", undefined, "en-GB")).toBe("No braces");
  });

  it("fill a name that looks like a placeholder as plain text, once", () => {
    expect(interpolate("{{a}} and {{b}}", { a: "{{b}}", b: "B" }, "en-GB")).toBe("{{b}} and B");
  });

  it("are listed once each, in order", () => {
    expect(placeholders("{{name}} has {{count}} skins, {{name}}")).toEqual(["name", "count"]);
    expect(placeholders("none here")).toEqual([]);
  });
});

describe("plurals", () => {
  it("use the categories each language's rules have", () => {
    expect(pluralCategories("en")).toEqual(["one", "other"]);
    expect(pluralCategories("ja")).toEqual(["other"]);
    expect(pluralCategories("fr")).toEqual(["one", "many", "other"]);
    expect(pluralCategory("en", 1)).toBe("one");
    expect(pluralCategory("en", 0)).toBe("other");
    expect(pluralCategory("fr", 0)).toBe("one");
    expect(pluralCategory("fr", 1_000_000)).toBe("many");
    expect(pluralCategory("ja", 1)).toBe("other");
  });

  it("pick the key for the count's category in the language on show", () => {
    const french = lookupIn("fr", "fr", {
      "folder.count_one": "{{count}} dossier",
      "folder.count_many": "{{count}} de dossiers",
      "folder.count_other": "{{count}} dossiers",
    });
    expect(translate(french, "folder.count", { count: 0 })).toBe("0 dossier");
    expect(translate(french, "folder.count", { count: 1 })).toBe("1 dossier");
    expect(translate(french, "folder.count", { count: 3 })).toBe("3 dossiers");
    expect(plain(translate(french, "folder.count", { count: 1_000_000 }))).toBe("1 000 000 de dossiers");
    const japanese = lookupIn("ja", "ja", { "folder.count_other": "{{count}} 個のフォルダ" });
    expect(translate(japanese, "folder.count", { count: 1 })).toBe("1 個のフォルダ");
  });

  it("fall back to the language's own other, then to English", () => {
    const noMany = lookupIn("fr", "fr", { "folder.count_one": "{{count}} dossier", "folder.count_other": "{{count}} dossiers" });
    expect(plain(translate(noMany, "folder.count", { count: 2_000_000 }))).toBe("2 000 000 dossiers");
    const none = lookupIn("fr", "fr", {});
    // English's own category for the count, its numbers written the French way.
    expect(translate(none, "folder.count", { count: 1 })).toBe("1 folder");
    expect(plain(translate(none, "folder.count", { count: 1234 }))).toBe("1 234 folders");
  });

  it("are found without a count too, by their other form", () => {
    expect(translate(lookupIn("en", "en-GB", english), "folder.count")).toBe("{{count}} folders");
  });
});

describe("a lookup", () => {
  it("finds the message in the language on show", () => {
    const french = lookupIn("fr", "fr", { "sidebar.addPhoto": "Ajouter une photo" });
    expect(translate(french, "sidebar.addPhoto")).toBe("Ajouter une photo");
  });

  it("falls back to English for a message the language doesn't have", () => {
    const french = lookupIn("fr", "fr", { "sidebar.addPhoto": "Ajouter une photo" });
    expect(translate(french, "sidebar.settings")).toBe("Settings");
    expect(translate(french, "folder.named", { name: "Voyages", count: 2 })).toBe("Voyages has 2 skins");
  });

  it("gives back a key no catalog has, and says so", () => {
    const missing = vi.fn();
    expect(translate(lookupIn("fr", "fr", {}), "sidebar.nothing", undefined, missing)).toBe("sidebar.nothing");
    expect(missing).toHaveBeenCalledWith("sidebar.nothing");
  });
});

describe("messages with markup", () => {
  it("split at their tags", () => {
    expect(richParts("Credited to <b>{{handle}}</b>.")).toEqual([{ text: "Credited to " }, { tag: "b", text: "{{handle}}" }, { text: "." }]);
    expect(richParts("Made with <brand/>")).toEqual([{ text: "Made with " }, { tag: "brand", text: "" }]);
    expect(richParts("no tags")).toEqual([{ text: "no tags" }]);
  });

  it("leave a tag that isn't closed as text", () => {
    expect(richParts("a <b>b")).toEqual([{ text: "a <b>b" }]);
  });

  it("list their tags once each", () => {
    expect(tags("<b>a</b> and <link>b</link> and <b>c</b>")).toEqual(["b", "link"]);
  });
});
