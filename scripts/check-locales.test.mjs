import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { LOCALES } from "../src/i18n/locales";
import { checkAll, checkEnglish, checkLocale, checkMessage, main, pluralCategories, readLocales } from "./check-locales.mjs";

const english = {
  sidebar: {
    add: "Add your photo",
    skins_one: "{{count}} skin",
    skins_other: "{{count}} skins",
    credit: "Credited to <b>{{handle}}</b>.",
  },
};

const french = {
  sidebar: {
    add: "Ajouter une photo",
    skins_one: "Un habillage",
    skins_many: "{{count}} d'habillages",
    skins_other: "{{count}} habillages",
    credit: "Crédité à <b>{{handle}}</b>.",
  },
};

const errors = (result) => result.problems.filter((p) => p.level === "error");
const messagesOf = (result) => result.problems.map((p) => `${p.key ?? p.namespace}: ${p.message}`);

describe("the languages and their plurals", () => {
  it("reads the same languages as the app", () => {
    expect(readLocales()).toEqual([...LOCALES]);
  });

  it("knows each language's plural forms from CLDR", () => {
    expect(pluralCategories("en")).toEqual(["one", "other"]);
    expect(pluralCategories("ja")).toEqual(["other"]);
    expect(pluralCategories("zh-CN")).toEqual(["other"]);
    expect(pluralCategories("ko")).toEqual(["other"]);
    expect(pluralCategories("fr")).toEqual(["one", "many", "other"]);
    expect(pluralCategories("es")).toEqual(["one", "many", "other"]);
  });
});

describe("a message on its own", () => {
  it("is fine when it's plain words, placeholders and whole tags", () => {
    expect(checkMessage("Crédité à <b>{{handle}}</b>, merci !")).toEqual([]);
    expect(checkMessage("Made with <brand/>")).toEqual([]);
    expect(checkMessage("1 < 2, and 3–39 letters")).toEqual([]);
  });

  it("can't be empty", () => {
    expect(checkMessage("  ")).toEqual(["is empty"]);
  });

  it("has no em dash, horizontal bar or semicolon, in any script", () => {
    for (const text of ["Un dossier — ou deux", "フォルダ――写真", "a –– b", "Hola; adiós", "你好；再见", "这样——那样"]) {
      expect(checkMessage(text), text).toHaveLength(1);
      expect(checkMessage(text)[0]).toMatch(/Rewrite the sentence/);
    }
  });

  it("has whole placeholders and closed tags", () => {
    expect(checkMessage("Bonjour {{name}")).toEqual(["has a broken placeholder. Write it as {{name}}, exactly as in English"]);
    expect(checkMessage("Crédité à <b>{{handle}}")).toEqual(["has a tag that isn't closed, or is closed in the wrong place"]);
    expect(checkMessage("Crédité à {{handle}}</b>")).toEqual(["has a tag that isn't closed, or is closed in the wrong place"]);
  });
});

describe("a language against English", () => {
  it("passes when it has every message, each plural form its language uses, and the same placeholders and tags", () => {
    const result = checkLocale("fr", english, french);
    expect(result.problems).toEqual([]);
    expect(result).toMatchObject({ messages: 3, translated: 3 });
    const japanese = { sidebar: { add: "写真を追加", skins_other: "{{count}} 個のスキン", credit: "<b>{{handle}}</b> のクレジット。" } };
    expect(checkLocale("ja", english, japanese).problems).toEqual([]);
  });

  it("warns about a namespace not translated yet, and fails on one under --strict", () => {
    const loose = checkLocale("fr", english, { sidebar: {} });
    expect(loose.problems).toEqual([expect.objectContaining({ level: "warning", namespace: "sidebar", untranslated: 3 })]);
    expect(loose.translated).toBe(0);
    expect(errors(checkLocale("fr", english, { sidebar: {} }, { strict: true }))).toHaveLength(1);
  });

  it("fails on a namespace that's started but not finished", () => {
    const result = checkLocale("fr", english, { sidebar: { add: "Ajouter une photo" } });
    expect(messagesOf(result)).toEqual([
      'credit: is missing. English: "Credited to <b>{{handle}}</b>."',
      'skins_one: is missing. English: "{{count}} skins"',
      'skins_many: is missing. English: "{{count}} skins"',
      'skins_other: is missing. English: "{{count}} skins"',
    ]);
    expect(result.translated).toBe(1);
  });

  it("fails on a key English doesn't have, and a namespace English doesn't have", () => {
    const result = checkLocale("fr", english, { sidebar: { ...french.sidebar, ajouter: "Ajouter" }, extra: {} });
    expect(messagesOf(result)).toEqual([
      "extra: English has no namespace by this name. Remove the file",
      "ajouter: English has no message by this key. Remove it, or check its spelling",
    ]);
  });

  it("wants exactly the plural forms the language uses", () => {
    const { skins_many, ...noMany } = french.sidebar;
    expect(skins_many).toBeDefined();
    expect(messagesOf(checkLocale("fr", english, { sidebar: noMany }))).toEqual(['skins_many: is missing. English: "{{count}} skins"']);

    const japanese = { add: "写真を追加", skins_one: "1 個のスキン", skins_other: "{{count}} 個のスキン", credit: "<b>{{handle}}</b> のクレジット。" };
    expect(messagesOf(checkLocale("ja", english, { sidebar: japanese }))).toEqual([
      'skins_one: ja has no "one" plural form. It uses _other. Remove this one',
    ]);

    const unsplit = { add: "写真を追加", skins: "{{count}} 個のスキン", credit: "<b>{{handle}}</b> のクレジット。" };
    expect(messagesOf(checkLocale("ja", english, { sidebar: unsplit }))).toEqual([
      "skins_other: is missing. It's a plural: write skins_other, not skins",
    ]);
  });

  it("wants a message that isn't a plural in English not to be one", () => {
    const { add, ...rest } = french.sidebar;
    expect(add).toBeDefined();
    const result = checkLocale("fr", english, { sidebar: { ...rest, add_one: "Ajouter", add_other: "Ajouter" } });
    expect(messagesOf(result)).toEqual(["add: isn't a plural in English. Write it as add, with no _one"]);
  });

  it("wants the same placeholders, untranslated", () => {
    const result = checkLocale("fr", english, { sidebar: { ...french.sidebar, credit: "Crédité à <b>{{pseudo}}</b>." } });
    expect(messagesOf(result)).toEqual([
      "credit: uses the placeholders {{pseudo}}, and English uses {{handle}}. Keep them exactly, untranslated",
    ]);
  });

  it("lets a plural form other than 'other' leave the count out, but nothing else", () => {
    expect(checkLocale("fr", english, french).problems).toEqual([]);
    const noCount = checkLocale("fr", english, { sidebar: { ...french.sidebar, skins_other: "Des habillages" } });
    expect(messagesOf(noCount)).toEqual(["skins_other: leaves out {{count}}, which English uses"]);
    const extra = checkLocale("fr", english, { sidebar: { ...french.sidebar, skins_one: "{{count}} habillage de {{name}}" } });
    expect(messagesOf(extra)).toEqual(["skins_one: uses {{name}}, which English doesn't. Keep the placeholders exactly, untranslated"]);
  });

  it("wants the same tags", () => {
    const result = checkLocale("fr", english, { sidebar: { ...french.sidebar, credit: "Crédité à {{handle}}." } });
    expect(messagesOf(result)).toEqual([
      "credit: uses the tags none, and English uses <b>. Keep each tag, with its closing tag, around the words it goes with",
    ]);
  });

  it("checks every message on its own too", () => {
    const result = checkLocale("fr", english, { sidebar: { ...french.sidebar, add: "Ajouter une photo ; ou deux" } });
    expect(messagesOf(result)).toEqual(["add: has a semicolon (;). Rewrite the sentence without it"]);
  });

  it("fails on a file that's missing, isn't JSON, or holds something other than strings", () => {
    expect(messagesOf(checkLocale("fr", english, {}))).toEqual(["sidebar: is missing. Copy it from English, or write {} until it's translated"]);
    expect(messagesOf(checkLocale("fr", english, { sidebar: new Error("Unexpected token") }))).toEqual([
      "sidebar: isn't valid JSON: Unexpected token",
    ]);
    const odd = checkLocale("fr", english, { sidebar: { ...french.sidebar, add: 3, credit: ["a"] } });
    expect(messagesOf(odd)).toEqual(
      expect.arrayContaining(["add: has to be a string, not number", "credit: has to be a string, not a list"]),
    );
  });
});

describe("English", () => {
  it("has plurals with exactly one and other, and messages that pass on their own", () => {
    expect(checkEnglish(english)).toEqual([]);
    const problems = checkEnglish({ sidebar: { skins_other: "{{count}} skins", note: "One; two" } });
    expect(problems.map((p) => `${p.key}: ${p.message}`)).toEqual([
      "note: has a semicolon (;). Rewrite the sentence without it",
      "skins: an English plural has exactly _one and _other",
    ]);
  });
});

describe("every language on disk", () => {
  let dir;
  afterEach(() => {
    if (dir) rmSync(dir, { recursive: true, force: true });
    dir = undefined;
  });

  const write = (locale, namespaces) => {
    mkdirSync(join(dir, locale), { recursive: true });
    for (const [name, tree] of Object.entries(namespaces)) {
      writeFileSync(join(dir, locale, `${name}.json`), typeof tree === "string" ? tree : JSON.stringify(tree));
    }
  };

  it("checks each language's folder, and folders that aren't a language", () => {
    dir = mkdtempSync(join(tmpdir(), "check-locales-"));
    write("en", english);
    write("fr", french);
    write("ja", { sidebar: "{ not json" });
    write("de", { sidebar: {} });
    const { problems, coverage } = checkAll({ dir, locales: ["en", "fr", "ja", "es"] });
    expect(problems.map((p) => `${p.locale} ${p.namespace ?? ""}: ${p.message}`)).toEqual([
      "de : isn't one of the app's languages (src/i18n/locales.ts lists en, fr, ja, es)",
      expect.stringMatching(/^ja sidebar: isn't valid JSON/),
      "es : has no folder. Make one with a {} file for each of English's namespaces",
    ]);
    expect(coverage.fr).toEqual({ messages: 3, translated: 3 });
  });

  it("finds nothing wrong with the app's own catalogs", () => {
    const { problems } = checkAll();
    expect(problems.filter((p) => p.level === "error")).toEqual([]);
  });

  it("exits 1 on a problem, 0 without one, and 2 for a language it doesn't know", () => {
    const said = [];
    expect(main([], (line) => said.push(line))).toBe(0);
    expect(said.at(-1)).toMatch(/^\nNo problems/);
    expect(main(["--locale", "de"], (line) => said.push(line))).toBe(2);
    expect(main(["--strict", "--locale=fr"], () => {})).toBe(checkAll({ only: ["fr"], strict: true }).problems.length ? 1 : 0);
  });
});
