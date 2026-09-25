import { afterEach, describe, expect, it, vi } from "vitest";
import { ACCENTS, applyPrefs, readPrefs, reducesMotion, setPrefs } from "./prefs";

describe("appearance preferences", () => {
  it("read back what was saved, and the defaults for anything else", () => {
    expect(readPrefs({ accent: "purple", motion: "reduced" })).toEqual({ accent: "purple", motion: "reduced", language: null });
    expect(readPrefs({ accent: "chartreuse", motion: "wild" })).toEqual({ accent: "blue", motion: "system", language: null });
    expect(readPrefs(null)).toEqual({ accent: "blue", motion: "system", language: null });
    expect(readPrefs("blue")).toEqual({ accent: "blue", motion: "system", language: null });
    expect(readPrefs({ accent: "graphite" }).accent).toBe("mono");
  });

  it("keep a language the app speaks, and follow the computer's for anything else", () => {
    expect(readPrefs({ language: "ja" }).language).toBe("ja");
    expect(readPrefs({ language: "zh-CN" }).language).toBe("zh-CN");
    expect(readPrefs({ language: "de" }).language).toBeNull();
    expect(readPrefs({ language: 3 }).language).toBeNull();
  });

  it("set the attributes the stylesheets read, and none for the defaults", () => {
    const root = { dataset: {} as Record<string, string> } as unknown as HTMLElement;
    applyPrefs({ accent: "green", motion: "reduced", language: "fr" }, root);
    expect(root.dataset).toEqual({ accent: "green", motion: "reduced" });
    applyPrefs({ accent: "blue", motion: "system", language: null }, root);
    expect(root.dataset).toEqual({});
  });

  it("offer each accent once, blue first", () => {
    expect(ACCENTS[0].id).toBe("blue");
    expect(new Set(ACCENTS.map((a) => a.id)).size).toBe(ACCENTS.length);
  });

  describe("keeping still", () => {
    afterEach(() => vi.unstubAllGlobals());

    const computerAsks = (reduce: boolean) => {
      vi.stubGlobal("document", { documentElement: { dataset: {} } });
      vi.stubGlobal("matchMedia", (q: string) => ({ matches: reduce && q.includes("reduce") }));
    };

    it("follows Motion: Reduced even when the computer doesn't ask for it", () => {
      computerAsks(false);
      setPrefs({ motion: "system" });
      expect(reducesMotion()).toBe(false);
      setPrefs({ motion: "reduced" });
      expect(reducesMotion()).toBe(true);
    });

    it("follows the computer's own setting under System", () => {
      computerAsks(true);
      setPrefs({ motion: "system" });
      expect(reducesMotion()).toBe(true);
    });
  });
});
