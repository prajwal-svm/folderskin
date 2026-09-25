import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const setLanguage = vi.fn(async (_language: string, _menu: Record<string, string>, _pin: boolean) => {});
vi.mock("../lib/tauri", () => ({ api: { setLanguage } }));

/** A computer asking for `languages`, with nothing saved yet unless `saved` is. */
function computer(languages: string[], saved?: Record<string, unknown>) {
  const store = new Map<string, string>();
  if (saved) store.set("folderskin.prefs", JSON.stringify(saved));
  const root = { dataset: {} as Record<string, string>, lang: "" };
  vi.stubGlobal("localStorage", {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
  });
  vi.stubGlobal("document", { documentElement: root });
  vi.stubGlobal("navigator", { languages, language: languages[0] });
  return { store, root };
}

describe("the language the app speaks", () => {
  beforeEach(() => {
    vi.resetModules();
    setLanguage.mockClear();
  });
  afterEach(() => vi.unstubAllGlobals());

  it("is the computer's at first, when the app speaks it", async () => {
    const { root } = computer(["de-DE", "ja-JP", "en-US"]);
    const { startLanguage, startingLocale } = await import("./language");
    const { getLocale } = await import("../i18n");
    expect(startingLocale()).toBe("ja");
    await startLanguage();
    expect(getLocale()).toBe("ja");
    expect(root.lang).toBe("ja");
    // The app hears it for its menu bar, not pinned: the computer's language may change.
    expect(setLanguage).toHaveBeenCalledWith("ja", expect.objectContaining({ copy: "Copy", quit: "Quit FolderSkin" }), false);
  });

  it("is English when the app speaks none of the computer's", async () => {
    const { root } = computer(["de-DE", "pt-BR"]);
    const { startLanguage } = await import("./language");
    const { getLocale } = await import("../i18n");
    await startLanguage();
    expect(getLocale()).toBe("en");
    expect(root.lang).toBe("en");
  });

  it("is the one chosen, from then on, whatever the computer's is", async () => {
    const { store, root } = computer(["fr-FR"]);
    const { chooseLanguage } = await import("./language");
    const { getLocale } = await import("../i18n");
    await chooseLanguage("ko");
    expect(getLocale()).toBe("ko");
    expect(root.lang).toBe("ko");
    expect(JSON.parse(store.get("folderskin.prefs") ?? "{}")).toMatchObject({ language: "ko" });
    expect(setLanguage).toHaveBeenLastCalledWith("ko", expect.any(Object), true);

    // The next launch.
    vi.resetModules();
    const next = await import("./language");
    const i18n = await import("../i18n");
    expect(next.startingLocale()).toBe("ko");
    await next.startLanguage();
    expect(i18n.getLocale()).toBe("ko");
  });

  it("sends the menu bar every word it needs", async () => {
    computer(["en-GB"]);
    const { menuWords } = await import("./language");
    const words = menuWords();
    expect(Object.keys(words)).toHaveLength(21);
    expect(Object.values(words).every((w) => w.trim().length > 0)).toBe(true);
  });
});
