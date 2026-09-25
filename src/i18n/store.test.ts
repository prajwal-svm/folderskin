import { describe, expect, it, vi } from "vitest";
import { createI18n, type Namespaces } from "./store";

const english: Namespaces = { sidebar: { addPhoto: "Add your photo", settings: "Settings", count_one: "{{count}} skin", count_other: "{{count}} skins" } };
const french: Namespaces = { sidebar: { addPhoto: "Ajouter une photo" } };
const japanese: Namespaces = { sidebar: { addPhoto: "写真を追加", settings: "設定" } };

/** A loader that resolves when the test says so. */
function deferred(namespaces: Namespaces) {
  let resolve: () => void = () => {};
  const loader = vi.fn(() => new Promise<Namespaces>((ok) => (resolve = () => ok(namespaces))));
  return { loader, resolve: () => resolve() };
}

describe("the language on show", () => {
  it("starts in English, with nothing else loaded", () => {
    const fr = vi.fn(async () => french);
    const i18n = createI18n({ english, loaders: { fr } });
    expect(i18n.locale()).toBe("en");
    expect(i18n.t()("sidebar.addPhoto")).toBe("Add your photo");
    expect(i18n.t()("sidebar.count", { count: 1 })).toBe("1 skin");
    expect(fr).not.toHaveBeenCalled();
  });

  it("loads another language the first time it's asked for, then shows it", async () => {
    const fr = vi.fn(async () => french);
    const i18n = createI18n({ english, loaders: { fr } });
    await i18n.setLocale("fr");
    expect(i18n.locale()).toBe("fr");
    expect(i18n.t()("sidebar.addPhoto")).toBe("Ajouter une photo");
    // Anything it doesn't have is in English.
    expect(i18n.t()("sidebar.settings")).toBe("Settings");
    await i18n.setLocale("en");
    await i18n.setLocale("fr");
    expect(fr).toHaveBeenCalledTimes(1);
  });

  it("loads a language once, however many ask for it at the same time", async () => {
    const ja = deferred(japanese);
    const i18n = createI18n({ english, loaders: { ja: ja.loader } });
    const first = i18n.load("ja");
    const second = i18n.setLocale("ja");
    ja.resolve();
    await Promise.all([first, second]);
    expect(ja.loader).toHaveBeenCalledTimes(1);
    expect(i18n.t()("sidebar.settings")).toBe("設定");
  });

  it("switches all at once, and tells whoever listens", async () => {
    const ja = deferred(japanese);
    const i18n = createI18n({ english, loaders: { ja: ja.loader } });
    const heard = vi.fn();
    const stop = i18n.subscribe(heard);
    const before = i18n.t();
    const switching = i18n.setLocale("ja");
    // Still English, whole, while Japanese loads.
    expect(i18n.locale()).toBe("en");
    expect(i18n.t()).toBe(before);
    expect(heard).not.toHaveBeenCalled();
    ja.resolve();
    await switching;
    expect(heard).toHaveBeenCalledTimes(1);
    // A new `t`, so anything drawn with the old one draws again.
    expect(i18n.t()).not.toBe(before);
    stop();
    await i18n.setLocale("en");
    expect(heard).toHaveBeenCalledTimes(1);
  });

  it("says nothing when the language asked for is already on show", async () => {
    const i18n = createI18n({ english, loaders: {} });
    const heard = vi.fn();
    i18n.subscribe(heard);
    await i18n.setLocale("en");
    expect(heard).not.toHaveBeenCalled();
  });

  it("is the last one asked for, whichever loads first", async () => {
    const ja = deferred(japanese);
    const fr = deferred(french);
    const i18n = createI18n({ english, loaders: { ja: ja.loader, fr: fr.loader } });
    const toJapanese = i18n.setLocale("ja");
    const toFrench = i18n.setLocale("fr");
    fr.resolve();
    await toFrench;
    expect(i18n.locale()).toBe("fr");
    ja.resolve();
    await toJapanese;
    expect(i18n.locale()).toBe("fr");
    expect(i18n.t()("sidebar.addPhoto")).toBe("Ajouter une photo");
  });

  it("stays as it was when a language fails to load, and can try again", async () => {
    let fail = true;
    const ko = vi.fn(async () => {
      if (fail) throw new Error("offline");
      return { sidebar: { addPhoto: "사진 추가" } };
    });
    const i18n = createI18n({ english, loaders: { ko } });
    await expect(i18n.setLocale("ko")).rejects.toThrow("offline");
    expect(i18n.locale()).toBe("en");
    fail = false;
    await i18n.setLocale("ko");
    expect(i18n.t()("sidebar.addPhoto")).toBe("사진 추가");
  });

  it("can't be a language with nothing to load it from", async () => {
    const i18n = createI18n({ english, loaders: {} });
    await expect(i18n.setLocale("es")).rejects.toThrow("no messages for es");
    expect(i18n.locale()).toBe("en");
  });
});

describe("English that comes later", () => {
  it("is there for every lookup from then on, in every language", async () => {
    const onMissing = vi.fn();
    const i18n = createI18n({ english, loaders: { fr: async () => french }, onMissing });
    expect(i18n.t()("composer.title")).toBe("composer.title");
    expect(onMissing).toHaveBeenCalledWith("composer.title");
    i18n.addEnglish({ composer: { title: "Design your own" } });
    expect(i18n.t()("composer.title")).toBe("Design your own");
    await i18n.setLocale("fr");
    expect(i18n.t()("composer.title")).toBe("Design your own");
  });
});
