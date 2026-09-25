import { afterEach, describe, expect, it, vi } from "vitest";
import {
  defaultProfile,
  loadProfiles,
  makeDefault,
  MAX_PROFILE_NAME,
  MAX_PROFILES,
  profileProblem,
  readProfiles,
  removeProfile,
  upsertProfile,
  type LicenceProfile,
  type Profiles,
} from "./profiles";

const profile = (id: string, name = id, author = "", license: LicenceProfile["license"] = "CC0-1.0"): LicenceProfile => ({ id, name, author, license });
const two = (): Profiles => ({ list: [profile("a", "Personal", "jane"), profile("b", "For work", "acme-studio", "CC-BY-4.0")], defaultId: "a" });

describe("licence profiles", () => {
  it("start from what sharing remembered before there were profiles", () => {
    const p = readProfiles(null, { author: "jane", license: "MIT" });
    expect(p.list).toEqual([{ id: "personal", name: "Personal", author: "jane", license: "MIT" }]);
    expect(p.defaultId).toBe("personal");
    // Nothing remembered, or nonsense: one empty profile under CC0.
    expect(readProfiles(null, null).list[0]).toMatchObject({ author: "", license: "CC0-1.0" });
    expect(readProfiles(null, { author: "not a name!", license: "GPL" }).list[0]).toMatchObject({ author: "", license: "CC0-1.0" });
  });

  it("are read back sound: no repeats, known licences, a default that exists, never none", () => {
    const p = readProfiles({
      list: [profile("a", "  Personal  "), profile("a", "Again"), { id: "b", name: "", author: "bad name", license: "WTFPL" }, "junk", { name: "no id" }],
      defaultId: "gone",
    });
    expect(p.list.map((x) => x.id)).toEqual(["a", "b"]);
    expect(p.list[0].name).toBe("Personal");
    expect(p.list[1]).toMatchObject({ name: "Profile", author: "", license: "CC0-1.0" });
    expect(p.defaultId).toBe("a");
    expect(readProfiles({ list: [], defaultId: "x" }).list).toHaveLength(1);
    expect(readProfiles({ list: Array.from({ length: 30 }, (_, i) => profile(`p${i}`)) }).list).toHaveLength(MAX_PROFILES);
  });

  it("number a name used twice, so each can still be changed without renaming it", () => {
    const p = readProfiles({ list: [{ id: "x", name: "" }, { id: "y", name: "  " }, { id: "z", name: "profile" }], defaultId: "x" });
    expect(p.list.map((x) => x.name)).toEqual(["Profile", "Profile 2", "profile 3"]);
    // Changing one's licence alone finds nothing wrong.
    expect(profileProblem(p.list[1], p.list.filter((o) => o.id !== "y"))).toBeNull();
    const long = "L".repeat(MAX_PROFILE_NAME);
    const q = readProfiles({ list: [profile("a", long), profile("b", long)] });
    expect(q.list[1].name).toBe(`${"L".repeat(MAX_PROFILE_NAME - 2)} 2`);
  });

  it("say what's wrong before one is kept, and which field it's about", () => {
    const others = [profile("a", "Personal")];
    expect(profileProblem({ name: "  ", author: "" }, others)).toMatchObject({ field: "name", text: expect.stringMatching(/name/) });
    expect(profileProblem({ name: "personal", author: "" }, others)).toMatchObject({ field: "name", text: expect.stringMatching(/already/) });
    expect(profileProblem({ name: "Work", author: "two  words" }, others)).toMatchObject({ field: "author", text: expect.stringMatching(/letters, numbers and single dashes/) });
    expect(profileProblem({ name: "Work", author: "acme-studio" }, others)).toBeNull();
    expect(profileProblem({ name: "Work", author: "" }, others)).toBeNull();
  });

  it("say so when a new one would be one too many", () => {
    const full = Array.from({ length: MAX_PROFILES }, (_, i) => profile(`p${i}`));
    expect(profileProblem({ name: "Brand new", author: "" }, full)).toMatchObject({ field: "list" });
    // Changing one of a full list is fine: the others are one fewer.
    expect(profileProblem({ name: "p0", author: "" }, full.slice(1))).toBeNull();
  });

  it("are added, changed, made the default and deleted, keeping at least one", () => {
    let p = upsertProfile(two(), profile("c", "  Side  projects ", "jane", "MIT"));
    expect(p.list.map((x) => x.name)).toEqual(["Personal", "For work", "Side projects"]);
    p = upsertProfile(p, { ...p.list[1], license: "MIT" });
    expect(p.list[1].license).toBe("MIT");
    p = makeDefault(p, "b");
    expect(defaultProfile(p).name).toBe("For work");
    expect(makeDefault(p, "nope")).toBe(p);
    // Deleting the default hands it to the first left.
    p = removeProfile(p, "b");
    expect(p.defaultId).toBe("a");
    p = removeProfile(removeProfile(p, "c"), "a");
    expect(p.list.map((x) => x.id)).toEqual(["a"]);
  });

  it("stop at the most there can be", () => {
    let p: Profiles = { list: Array.from({ length: MAX_PROFILES }, (_, i) => profile(`p${i}`)), defaultId: "p0" };
    const before = p;
    p = upsertProfile(p, profile("one-more"));
    expect(p).toBe(before);
  });

  describe("from this computer", () => {
    afterEach(() => vi.unstubAllGlobals());
    const stored = (items: Record<string, string>) => vi.stubGlobal("localStorage", { getItem: (k: string) => items[k] ?? null });

    it("fall back to what sharing kept before when the profiles are broken", () => {
      stored({ "folderskin.sharing.profiles": "{not json", "folderskin.sharing": JSON.stringify({ author: "jane", license: "MIT" }) });
      expect(loadProfiles().list).toEqual([{ id: "personal", name: "Personal", author: "jane", license: "MIT" }]);
    });

    it("start empty when neither can be read", () => {
      stored({ "folderskin.sharing.profiles": "{not json", "folderskin.sharing": "{nor this" });
      expect(loadProfiles().list[0]).toMatchObject({ name: "Personal", author: "", license: "CC0-1.0" });
    });
  });
});
