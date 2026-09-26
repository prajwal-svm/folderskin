import { describe, expect, it } from "vitest";
import { initialState, insideCount, reduce, treeOnly, type Action, type State, type SubfolderChoice } from "./dropzone";
import { loadFavorites, saveFavorites, toggleFavorite, type KeyValueStore } from "./favorites";
import { browseLabel } from "../lib/platform";

const readme = { path: "/Users/me/Desktop/readme", name: "readme" };

function run(actions: Action[], from: State = initialState): State {
  return actions.reduce(reduce, from);
}

describe("drop zone state machine", () => {
  it("starts idle with no folder", () => {
    expect(initialState.phase).toBe("idle");
    expect(initialState.folder).toBeNull();
  });

  it("dropping a folder shows the folder state until a skin is picked", () => {
    const s = run([{ type: "folderDropped", folder: readme }]);
    expect(s.phase).toBe("folder");
    expect(s.folder).toEqual(readme);
  });

  it("folder + skin is ready to apply, in either order", () => {
    const a = run([{ type: "folderDropped", folder: readme }, { type: "skinSelected", skinId: "aurora" }]);
    const b = run([{ type: "skinSelected", skinId: "aurora" }, { type: "folderDropped", folder: readme }]);
    expect(a.phase).toBe("ready");
    expect(b.phase).toBe("ready");
  });

  it("selecting a skin without a folder stays idle but remembers the choice", () => {
    const s = run([{ type: "skinSelected", skinId: "mesh" }]);
    expect(s.phase).toBe("idle");
    expect(s.skinId).toBe("mesh");
  });

  it("apply goes through applying to applied and keeps the glow", () => {
    const ready = run([{ type: "folderDropped", folder: readme }, { type: "skinSelected", skinId: "aurora" }]);
    const applying = reduce(ready, { type: "applyStarted" });
    expect(applying.phase).toBe("applying");
    const applied = reduce(applying, { type: "applySucceeded" });
    expect(applied.phase).toBe("applied");
    expect(applied.appliedSkinId).toBe("aurora");
  });

  it("putting the skin down leaves the folder showing its own icon", () => {
    const ready = run([{ type: "folderDropped", folder: readme }, { type: "skinSelected", skinId: "aurora" }]);
    const s = reduce(ready, { type: "skinCleared" });
    expect(s.phase).toBe("folder");
    expect(s.skinId).toBeNull();
    const applying = reduce(ready, { type: "applyStarted" });
    expect(reduce(applying, { type: "skinCleared" })).toBe(applying);
  });

  it("cannot start applying without both a folder and a skin", () => {
    expect(reduce(initialState, { type: "applyStarted" })).toBe(initialState);
    const folderOnly = run([{ type: "folderDropped", folder: readme }]);
    expect(reduce(folderOnly, { type: "applyStarted" })).toBe(folderOnly);
  });

  it("picking a different skin after applying returns to ready; the same skin stays applied", () => {
    const applied = run([
      { type: "folderDropped", folder: readme },
      { type: "skinSelected", skinId: "aurora" },
      { type: "applyStarted" },
      { type: "applySucceeded" },
    ]);
    expect(reduce(applied, { type: "skinSelected", skinId: "sunset" }).phase).toBe("ready");
    expect(reduce(applied, { type: "skinSelected", skinId: "aurora" }).phase).toBe("applied");
  });

  it("a failed apply returns to ready with the message and keeps the folder", () => {
    const s = run([
      { type: "folderDropped", folder: readme },
      { type: "skinSelected", skinId: "aurora" },
      { type: "applyStarted" },
      { type: "applyFailed", message: "couldn't apply the skin: permission denied" },
    ]);
    expect(s.phase).toBe("ready");
    expect(s.error).toBe("couldn't apply the skin: permission denied");
    expect(s.folder).toEqual(readme);
    expect(reduce(s, { type: "clearError" }).error).toBeNull();
  });

  it("revert shows the folder with its default icon, skin put down", () => {
    const applied = run([
      { type: "folderDropped", folder: readme },
      { type: "skinSelected", skinId: "aurora" },
      { type: "applyStarted" },
      { type: "applySucceeded" },
    ]);
    const reverting = reduce(applied, { type: "revertStarted" });
    expect(reverting.phase).toBe("reverting");
    const done = reduce(reverting, { type: "revertSucceeded" });
    expect(done.phase).toBe("folder");
    expect(done.appliedSkinId).toBeNull();
    expect(done.skinId).toBeNull();
    expect(done.folder).toEqual(readme);
  });

  it("a failed revert stays applied and reports the error", () => {
    const s = run([
      { type: "folderDropped", folder: readme },
      { type: "skinSelected", skinId: "aurora" },
      { type: "applyStarted" },
      { type: "applySucceeded" },
      { type: "revertStarted" },
      { type: "revertFailed", message: "couldn't revert: permission denied" },
    ]);
    expect(s.phase).toBe("applied");
    expect(s.error).toContain("couldn't revert");
  });

  it("dropping a new folder while applied forgets the applied state", () => {
    const applied = run([
      { type: "folderDropped", folder: readme },
      { type: "skinSelected", skinId: "aurora" },
      { type: "applyStarted" },
      { type: "applySucceeded" },
    ]);
    const s = reduce(applied, { type: "folderDropped", folder: { path: "/tmp/other", name: "other" } });
    expect(s.phase).toBe("ready");
    expect(s.appliedSkinId).toBeNull();
  });

  it("a folder that replaces another shows its own icon first, then tries the skin on", () => {
    const other = { path: "/Users/me/Desktop/photos", name: "photos" };
    const trying = run([{ type: "folderDropped", folder: readme }, { type: "skinSelected", skinId: "aurora" }]);
    expect(trying.arriving).toBe(false);

    const arriving = reduce(trying, { type: "folderDropped", folder: other });
    expect(arriving.arriving).toBe(true);
    expect(arriving.phase).toBe("ready");
    expect(arriving.skinId).toBe("aurora");

    const arrived = reduce(arriving, { type: "arrived" });
    expect(arrived.arriving).toBe(false);
    expect(arrived.phase).toBe("ready");
  });

  it("the first folder, or one with no skin to try on, doesn't wait", () => {
    const first = run([{ type: "skinSelected", skinId: "aurora" }, { type: "folderDropped", folder: readme }]);
    expect(first.arriving).toBe(false);
    const noSkin = run([{ type: "folderDropped", folder: readme }, { type: "folderDropped", folder: { path: "/tmp/b", name: "b" } }]);
    expect(noSkin.arriving).toBe(false);
  });

  it("picking a skin or applying while a folder arrives shows the skin at once", () => {
    const arriving = run([
      { type: "folderDropped", folder: readme },
      { type: "skinSelected", skinId: "aurora" },
      { type: "folderDropped", folder: { path: "/tmp/b", name: "b" } },
    ]);
    expect(reduce(arriving, { type: "skinSelected", skinId: "sunset" }).arriving).toBe(false);
    expect(reduce(arriving, { type: "skinSelected", skinId: "aurora" }).arriving).toBe(false);
    expect(reduce(arriving, { type: "applyStarted" }).arriving).toBe(false);
    expect(reduce(arriving, { type: "skinCleared" }).arriving).toBe(false);
  });

  it("a folder's own custom icon can be removed while it shows, and comes back as it was if that fails", () => {
    const shown = run([{ type: "folderDropped", folder: readme }]);
    const removing = reduce(shown, { type: "revertStarted" });
    expect(removing.phase).toBe("reverting");
    expect(reduce(removing, { type: "revertSucceeded" }).phase).toBe("folder");
    const failed = reduce(removing, { type: "revertFailed", message: "couldn't: permission denied" });
    expect(failed.phase).toBe("folder");
    expect(failed.error).toContain("permission denied");
  });

  it("a folder waiting with its own icon can have it removed, which puts the skin down", () => {
    const waiting = run([
      { type: "folderDropped", folder: readme },
      { type: "skinSelected", skinId: "aurora" },
      { type: "folderDropped", folder: { path: "/tmp/b", name: "b" } },
    ]);
    const removing = reduce(waiting, { type: "revertStarted" });
    expect(removing.phase).toBe("reverting");
    const done = reduce(removing, { type: "revertSucceeded" });
    expect(done).toMatchObject({ phase: "folder", skinId: null, arriving: false });
    // Trying a skin on, nothing is on show to remove.
    const trying = reduce(waiting, { type: "arrived" });
    expect(reduce(trying, { type: "revertStarted" })).toBe(trying);
  });

  it("an invalid drop only sets the error and clears the drag", () => {
    const hovering = reduce(initialState, { type: "drag", info: { kind: "folder", name: "readme" } });
    expect(hovering.drag).toEqual({ kind: "folder", name: "readme" });
    const s = reduce(hovering, { type: "invalidDrop", message: "that's not a folder or a picture" });
    expect(s.phase).toBe("idle");
    expect(s.drag).toBeNull();
    expect(s.error).toBe("that's not a folder or a picture");
  });
});

describe("a folder and its subfolders", () => {
  const projects = { path: "/Users/me/Projects", name: "Projects" };
  const counted = (count: number, more = false): Action => ({ type: "subfoldersCounted", path: projects.path, subfolders: { count, more } });
  const ready = () => run([{ type: "folderDropped", folder: projects }, { type: "skinSelected", skinId: "coral" }, counted(24)]);
  const tree = (patch: Partial<NonNullable<State["run"]>> = {}): NonNullable<State["run"]> => ({
    kind: "apply",
    total: 25,
    changed: ["/Users/me/Projects", "/Users/me/Projects/a"],
    failed: [],
    skipped: 0,
    remaining: [],
    stopped: false,
    ...patch,
  });

  it("counts the subfolders of the folder on show, and no other", () => {
    const s = run([{ type: "folderDropped", folder: projects }, { type: "subfoldersCounted", path: "/elsewhere", subfolders: { count: 3, more: false } }]);
    expect(s.subfolders).toBeNull();
    expect(reduce(s, counted(24)).subfolders).toEqual({ count: 24, more: false });
  });

  it("includes subfolders only when there are some, and not too many", () => {
    const on = { type: "includeSubfolders", on: true } as const;
    expect(reduce(ready(), on).includeSubfolders).toBe(true);
    const none = run([{ type: "folderDropped", folder: projects }, counted(0)]);
    expect(reduce(none, on).includeSubfolders).toBe(false);
    const huge = run([{ type: "folderDropped", folder: projects }, counted(5000, true)]);
    expect(reduce(huge, on).includeSubfolders).toBe(false);
    // Not yet counted: nothing to include.
    expect(reduce(run([{ type: "folderDropped", folder: projects }]), on).includeSubfolders).toBe(false);
  });

  it("starts every new folder with only itself included", () => {
    const s = run([{ type: "includeSubfolders", on: true }, { type: "folderDropped", folder: readme }], ready());
    expect(s.includeSubfolders).toBe(false);
    expect(s.subfolders).toBeNull();
    expect(s.run).toBeNull();
  });

  it("can't change what's included while a run is going", () => {
    const applying = run([{ type: "includeSubfolders", on: true }, { type: "applyStarted" }], ready());
    expect(reduce(applying, { type: "includeSubfolders", on: false }).includeSubfolders).toBe(true);
  });

  it("follows a run's progress and keeps what it did", () => {
    const applying = run([{ type: "includeSubfolders", on: true }, { type: "applyStarted" }], ready());
    const going = reduce(applying, { type: "treeProgress", progress: { done: 12, total: 25, name: "Photos" } });
    expect(going.progress).toEqual({ done: 12, total: 25, name: "Photos" });
    const done = reduce(going, { type: "applySucceeded", run: tree() });
    expect(done.phase).toBe("applied");
    expect(done.progress).toBeNull();
    expect(done.run?.changed).toHaveLength(2);
    // Progress means nothing outside a run.
    expect(reduce(done, { type: "treeProgress", progress: { done: 1, total: 2, name: "x" } }).progress).toBeNull();
  });

  it("leaves the folder as it was when a run stopped before changing anything", () => {
    const applying = run([{ type: "includeSubfolders", on: true }, { type: "applyStarted" }], ready());
    const s = reduce(applying, { type: "applySucceeded", run: tree({ changed: [], stopped: true, remaining: ["/a"] }) });
    expect(s.phase).toBe("ready");
    expect(s.appliedSkinId).toBeNull();
    expect(s.run?.stopped).toBe(true);
  });

  it("forgets a run's summary when another skin is picked, or when dismissed", () => {
    const done = run([{ type: "applyStarted" }, { type: "applySucceeded", run: tree() }], ready());
    expect(reduce(done, { type: "skinSelected", skinId: "coral" }).run).not.toBeNull();
    expect(reduce(done, { type: "skinSelected", skinId: "mint" }).run).toBeNull();
    expect(reduce(done, { type: "runDismissed" }).run).toBeNull();
  });

  const choice = (paths: string[], total = 24): SubfolderChoice => ({ root: projects.path, paths: paths.map((p) => `${projects.path}/${p}`), total });
  const chose = (chosen: SubfolderChoice | null, total = 24, path = projects.path): Action => ({ type: "subfoldersChosen", path, total, chosen });

  it("takes the folders chosen inside, and counts them", () => {
    const on = reduce(ready(), { type: "includeSubfolders", on: true });
    expect(insideCount(on)).toBe(24);
    expect(treeOnly(on)).toBeNull();
    const some = reduce(on, chose(choice(["a", "b", "a/c"])));
    expect(insideCount(some)).toBe(3);
    expect(treeOnly(some)).toEqual(["/Users/me/Projects", "/Users/me/Projects/a", "/Users/me/Projects/b", "/Users/me/Projects/a/c"]);
    // Switched off, the folder goes alone, and the choice waits for the switch to come on again.
    const off = reduce(some, { type: "includeSubfolders", on: false });
    expect(insideCount(off)).toBe(0);
    expect(treeOnly(off)).toBeNull();
    expect(insideCount(reduce(off, { type: "includeSubfolders", on: true }))).toBe(3);
  });

  it("goes back to the whole tree when every folder is ticked, and to the folder alone when none is", () => {
    const some = run([{ type: "includeSubfolders", on: true }, chose(choice(["a"]))], ready());
    const all = reduce(some, chose(null, 26));
    expect(all.chosen).toBeNull();
    expect(all.includeSubfolders).toBe(true);
    expect(all.subfolders).toEqual({ count: 26, more: false });
    expect(insideCount(all)).toBe(26);
    const none = reduce(some, chose(choice([])));
    expect(none.chosen).toBeNull();
    expect(none.includeSubfolders).toBe(false);
    expect(insideCount(none)).toBe(0);
  });

  it("keeps the choice while the same folder is chosen, and forgets it for another", () => {
    const some = run([{ type: "includeSubfolders", on: true }, chose(choice(["a"]))], ready());
    const again = run([{ type: "folderDropped", folder: projects }, counted(24)], some);
    expect(again.chosen?.paths).toEqual(["/Users/me/Projects/a"]);
    expect(again.includeSubfolders).toBe(false);
    expect(reduce(some, { type: "folderDropped", folder: readme }).chosen).toBeNull();
    expect(reduce(some, { type: "folderCleared" }).chosen).toBeNull();
    // Chosen for a folder that isn't on show any more, or while a run is going: nothing changes.
    expect(reduce(ready(), chose(choice(["a"]), 24, readme.path)).chosen).toBeNull();
    const applying = run([{ type: "includeSubfolders", on: true }, { type: "applyStarted" }], ready());
    expect(reduce(applying, chose(choice(["a"])))).toBe(applying);
  });

  it("reports a revert over the tree the same way", () => {
    const done = run([{ type: "applyStarted" }, { type: "applySucceeded", run: tree() }, { type: "revertStarted" }], ready());
    expect(done.phase).toBe("reverting");
    const back = reduce(done, { type: "revertSucceeded", run: tree({ kind: "revert" }) });
    expect(back.phase).toBe("folder");
    expect(back.run?.kind).toBe("revert");
  });
});

describe("favorites", () => {
  function fakeStore(): KeyValueStore & { data: Record<string, string> } {
    const data: Record<string, string> = {};
    return { data, getItem: (k) => data[k] ?? null, setItem: (k, v) => void (data[k] = v) };
  }

  it("toggles ids on and off", () => {
    expect(toggleFavorite([], "aurora")).toEqual(["aurora"]);
    expect(toggleFavorite(["aurora", "mesh"], "aurora")).toEqual(["mesh"]);
  });

  it("round-trips through the store and tolerates garbage", () => {
    const store = fakeStore();
    saveFavorites(["aurora"], store);
    expect(loadFavorites(store)).toEqual(["aurora"]);
    store.setItem("folderskin.favorites", "{not json");
    expect(loadFavorites(store)).toEqual([]);
    expect(loadFavorites(null)).toEqual([]);
  });
});

describe("platform copy", () => {
  it("names the machine per OS", () => {
    expect(browseLabel("macos")).toBe("your Mac");
    expect(browseLabel("windows")).toBe("your PC");
    expect(browseLabel("linux")).toBe("your computer");
  });
});
