import { afterEach, describe, expect, it, vi } from "vitest";
import { creditDefaultProfile, isGithubUser, licenseLabel } from "./packs";
import { loadProfiles, PROFILES_KEY, saveProfiles, type Profiles } from "./profiles";

/** A localStorage of its own for a test: they run in Node, which has none. */
function stubStorage() {
  const kept = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (k: string) => kept.get(k) ?? null,
    setItem: (k: string, v: string) => void kept.set(k, v),
    removeItem: (k: string) => void kept.delete(k),
  });
  return kept;
}

describe("packs", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  // The same cases as `ids_file_names_and_users_cannot_escape_their_url` in pack.rs.
  it("take a name of letters, digits and single dashes as the author", () => {
    expect(isGithubUser("prajwal-svm")).toBe(true);
    for (const bad of ["", "-x", "x-", "a--b", "a b", "a".repeat(40)]) expect(isGithubUser(bad), bad).toBe(false);
  });

  it("name their licence the short way", () => {
    expect(licenseLabel("CC-BY-4.0")).toBe("CC BY 4.0");
    expect(licenseLabel("CC0-1.0")).toBe("CC0");
    expect(licenseLabel("MIT")).toBe("MIT");
  });

  it("leave the licence profiles as they were when one is shared", () => {
    const kept = stubStorage();
    const profiles: Profiles = {
      list: [
        { id: "a", name: "Personal", author: "jane", license: "CC0-1.0" },
        { id: "b", name: "For work", author: "acme-studio", license: "CC-BY-4.0" },
      ],
      defaultId: "b",
    };
    saveProfiles(profiles);
    const before = kept.get(PROFILES_KEY);
    creditDefaultProfile("jane");
    expect(kept.get(PROFILES_KEY)).toBe(before);
  });

  it("give a default that credits no one the name a pack was saved under, and nothing else", () => {
    stubStorage();
    saveProfiles({
      list: [
        { id: "a", name: "Personal", author: "", license: "MIT" },
        { id: "b", name: "For work", author: "", license: "CC-BY-4.0" },
      ],
      defaultId: "a",
    });
    creditDefaultProfile("not a name");
    expect(loadProfiles().list[0].author).toBe("");
    creditDefaultProfile("jane");
    expect(loadProfiles().list).toEqual([
      { id: "a", name: "Personal", author: "jane", license: "MIT" },
      { id: "b", name: "For work", author: "", license: "CC-BY-4.0" },
    ]);
  });
});
