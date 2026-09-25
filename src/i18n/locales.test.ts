import { describe, expect, it } from "vitest";
import { detectLocale, docsUrl, isLocale, LOCALE_NAMES, LOCALES, matchLocale } from "./locales";

describe("the languages", () => {
  it("are six, English first, each named in itself", () => {
    expect(LOCALES).toEqual(["en", "zh-CN", "ja", "ko", "fr", "es"]);
    expect(Object.values(LOCALE_NAMES)).toEqual(["English", "简体中文", "日本語", "한국어", "Français", "Español"]);
  });

  it("are told apart from anything else", () => {
    expect(isLocale("ja")).toBe(true);
    expect(isLocale("zh-CN")).toBe(true);
    expect(isLocale("zh")).toBe(false);
    expect(isLocale("de")).toBe(false);
    expect(isLocale(undefined)).toBe(false);
  });
});

describe("a computer's language", () => {
  it("is matched by language, whatever the region", () => {
    expect(matchLocale("fr-CA")).toBe("fr");
    expect(matchLocale("es-419")).toBe("es");
    expect(matchLocale("ja-JP")).toBe("ja");
    expect(matchLocale("ko-KR")).toBe("ko");
    expect(matchLocale("en-US")).toBe("en");
    expect(matchLocale(" FR_fr ")).toBe("fr");
  });

  it("is Simplified Chinese for any Chinese", () => {
    for (const tag of ["zh", "zh-CN", "zh-Hans", "zh-Hans-CN", "zh-TW", "zh-Hant-HK", "zh_SG"]) {
      expect(matchLocale(tag), tag).toBe("zh-CN");
    }
  });

  it("isn't matched when the app doesn't speak it", () => {
    expect(matchLocale("de-DE")).toBeNull();
    expect(matchLocale("pt-BR")).toBeNull();
    expect(matchLocale("")).toBeNull();
  });

  it("picks the first language the app speaks, and English when there's none", () => {
    expect(detectLocale(["de-DE", "fr-FR", "ja-JP"])).toBe("fr");
    expect(detectLocale(["zh-Hant-TW", "en-US"])).toBe("zh-CN");
    expect(detectLocale(["de-DE", "pt-BR"])).toBe("en");
    expect(detectLocale([])).toBe("en");
    expect(detectLocale(undefined)).toBe("en");
  });
});

describe("the guides", () => {
  it("are on the website in each language", () => {
    expect(docsUrl("packs", "en")).toBe("https://folderskin.app/docs/packs/");
    expect(docsUrl("packs", "zh-CN")).toBe("https://folderskin.app/zh-cn/docs/packs/");
    expect(docsUrl("/ai/", "ja")).toBe("https://folderskin.app/ja/docs/ai/");
    expect(docsUrl("composer", "ko")).toBe("https://folderskin.app/ko/docs/composer/");
    expect(docsUrl("skins", "fr")).toBe("https://folderskin.app/fr/docs/skins/");
    expect(docsUrl("pack-terms", "es")).toBe("https://folderskin.app/es/docs/pack-terms/");
  });
});
