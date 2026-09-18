import { describe, expect, it } from "vitest";
import { buttonLabel, hasGlow, initialState, reduce, type Action, type State } from "./dropzone";
import { loadFavorites, saveFavorites, toggleFavorite, type KeyValueStore } from "./favorites";
import { browseLabel } from "../lib/platform";

const readme = { path: "/Users/me/Desktop/readme", name: "readme" };

function run(actions: Action[], from: State = initialState): State {
  return actions.reduce(reduce, from);
}

describe("drop zone state machine", () => {
  it("starts idle with no folder and no button", () => {
    expect(initialState.phase).toBe("idle");
    expect(buttonLabel(initialState)).toBeNull();
    expect(hasGlow(initialState)).toBe(false);
  });

  it("dropping a folder shows the folder state until a skin is picked", () => {
    const s = run([{ type: "folderDropped", folder: readme }]);
    expect(s.phase).toBe("folder");
    expect(s.folder).toEqual(readme);
    expect(buttonLabel(s)).toBeNull();
  });

  it("folder + skin is ready to apply, in either order", () => {
    const a = run([{ type: "folderDropped", folder: readme }, { type: "skinSelected", skinId: "aurora" }]);
    const b = run([{ type: "skinSelected", skinId: "aurora" }, { type: "folderDropped", folder: readme }]);
    expect(a.phase).toBe("ready");
    expect(b.phase).toBe("ready");
    expect(buttonLabel(a)).toBe("Apply skin");
    expect(hasGlow(a)).toBe(true);
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
    expect(buttonLabel(applying)).toBe("Applying…");
    const applied = reduce(applying, { type: "applySucceeded" });
    expect(applied.phase).toBe("applied");
    expect(applied.appliedSkinId).toBe("aurora");
    expect(buttonLabel(applied)).toBe("Applied");
    expect(hasGlow(applied)).toBe(true);
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

  it("revert goes back to ready with folder and skin kept", () => {
    const applied = run([
      { type: "folderDropped", folder: readme },
      { type: "skinSelected", skinId: "aurora" },
      { type: "applyStarted" },
      { type: "applySucceeded" },
    ]);
    const reverting = reduce(applied, { type: "revertStarted" });
    expect(reverting.phase).toBe("reverting");
    expect(buttonLabel(reverting)).toBe("Reverting…");
    const done = reduce(reverting, { type: "revertSucceeded" });
    expect(done.phase).toBe("ready");
    expect(done.appliedSkinId).toBeNull();
    expect(done.skinId).toBe("aurora");
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

  it("an invalid drop only sets the error and clears hover", () => {
    const hovering = reduce(initialState, { type: "drag", hover: true });
    expect(hovering.hover).toBe(true);
    const s = reduce(hovering, { type: "invalidDrop", message: "that's not a folder or a picture" });
    expect(s.phase).toBe("idle");
    expect(s.hover).toBe(false);
    expect(s.error).toBe("that's not a folder or a picture");
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
