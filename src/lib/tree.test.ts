import { describe, expect, it } from "vitest";
import { applyLabel, canCarryOn, canRetry, canUndo, doneOf, folders, formatBytes, isBig, knownTotal, remaining, runNotice, runSummary, runToast, shareOf, treeProgress, type TreeRun } from "./tree";

const run = (patch: Partial<TreeRun>): TreeRun => ({
  id: 1,
  kind: "apply",
  undoing: false,
  leaves_plain: false,
  folder: "/Users/me/Projects",
  root: "/Users/me/Projects",
  name: "Projects",
  skin_id: "user:coral",
  running: false,
  stopping: false,
  stopped: false,
  done: 25,
  total: 25,
  counted: true,
  current: "Raw",
  changed: 25,
  failed: 0,
  skipped: 0,
  failures: [],
  error: null,
  ...patch,
});
const failures = (count: number) => Array.from({ length: count }, (_, i) => ({ path: `/p/${i}`, name: `${i}`, reason: "no" }));

describe("runs over a folder and its subfolders", () => {
  it("counts folders the way people say them", () => {
    expect(folders(1)).toBe("1 folder");
    expect(folders(1234)).toBe("1,234 folders");
    expect(applyLabel(24)).toBe("Apply to 25 folders");
    expect(applyLabel(1)).toBe("Apply to 2 folders");
    expect(applyLabel(12_400, false)).toBe("Apply to 12,401+ folders");
  });

  it("says how far a run has got, with a + until every folder is found", () => {
    expect(doneOf(run({ done: 3120, total: 12_000, counted: false }))).toBe("3,120 of 12,000+");
    expect(doneOf(run({ done: 3120, total: 48_210 }))).toBe("3,120 of 48,210");
    expect(shareOf(run({ done: 3120, total: 12_000, counted: false }))).toBeNull();
    expect(shareOf(run({ done: 12_000, total: 48_000 }))).toBe(0.25);
  });

  it("takes how many there are from a count that had finished, until the run has found them itself", () => {
    const walking = run({ done: 1, total: 5, counted: false });
    expect(knownTotal(walking, 29)).toBe(29);
    expect(doneOf(walking, 29)).toBe("1 of 29");
    expect(shareOf(walking, 20)).toBe(0.05);
    // The walk has found more than the count did: the folder has grown since.
    expect(doneOf(run({ done: 1, total: 31, counted: false }), 29)).toBe("1 of 31+");
    // Once the walk is over, its own count is the one that's right.
    expect(doneOf(run({ done: 28, total: 28 }), 29)).toBe("28 of 28");
    expect(treeProgress(walking, 29)).toEqual({ done: 1, total: 29, counted: true });
    expect(treeProgress(walking, null)).toEqual({ done: 1, total: 5, counted: false });
  });

  it("asks in its own words before a big run, or one still being counted", () => {
    expect(isBig(4999, true)).toBe(false);
    expect(isBig(5000, true)).toBe(true);
    expect(isBig(30, false)).toBe(true);
  });

  it("writes sizes people can read", () => {
    expect(formatBytes(900)).toBe("900 bytes");
    expect(formatBytes(3_718_461)).toBe("3.7 MB");
    expect(formatBytes(92_961_525)).toBe("93 MB");
    expect(formatBytes(1_250_000_000)).toBe("1.3 GB");
  });

  it("sums up an apply that reached every folder", () => {
    expect(runSummary(run({}), "Coral")).toEqual({ title: "25 folders now wear Coral", detail: null, tone: "ok" });
    expect(runSummary(run({ total: 1, done: 1, changed: 1 }), "Coral").title).toBe("1 folder now wears Coral");
    expect(runSummary(run({}), null).title).toBe("25 folders now wear the skin");
  });

  it("says how many couldn't be changed", () => {
    const s = runSummary(run({ changed: 22, failed: 3, failures: failures(3) }), "Coral");
    expect(s).toEqual({ title: "22 of 25 folders now wear Coral", detail: "3 couldn't be changed.", tone: "warn" });
  });

  it("says where a stopped run got to, found or not", () => {
    const stopped = run({ stopped: true, done: 12, changed: 12 });
    expect(runSummary(stopped, "Coral")).toEqual({ title: "Stopped after 12 folders", detail: "Those 12 wear Coral. 13 weren't reached.", tone: "warn" });
    expect(remaining(stopped)).toBe(13);
    const early = run({ stopped: true, done: 12, changed: 12, total: 300, counted: false });
    expect(runSummary(early, "Coral").detail).toBe("Those 12 wear Coral. The rest weren't reached.");
    expect(remaining(early)).toBeNull();
    expect(runSummary(run({ stopped: true, done: 0, changed: 0 }), "Coral")).toEqual({ title: "Stopped before any folder was changed", detail: null, tone: "warn" });
  });

  it("says so when some of the folders a stopped run went through couldn't be changed", () => {
    const stopped = run({ stopped: true, done: 24_938, changed: 24_912, failed: 26, failures: failures(26), total: 101_041 });
    expect(runSummary(stopped, "Coral")).toEqual({
      title: "Stopped after 24,938 folders",
      detail: "24,912 of them wear Coral. 26 couldn't be changed. 76,103 weren't reached.",
      tone: "warn",
    });
    expect(runSummary(run({ stopped: true, done: 2, changed: 0, failed: 2 }), "Coral").detail).toBe("2 couldn't be changed.");
    const revert = run({ kind: "revert", stopped: true, done: 14, changed: 12, failed: 2 });
    expect(runSummary(revert, null).detail).toBe("Those 12 have the default icon back. 2 couldn't be changed.");
  });

  it("sums up a revert, with the folders it left alone", () => {
    expect(runSummary(run({ kind: "revert", leaves_plain: true, changed: 14, skipped: 11 }), null)).toEqual({
      title: "14 folders have the default icon back",
      detail: "11 folders had no icon of their own.",
      tone: "ok",
    });
    expect(runSummary(run({ kind: "revert", changed: 0, skipped: 25 }), null).title).toBe("No folder here has a custom icon");
    expect(runSummary(run({ kind: "revert", changed: 1 }), null).title).toBe("1 folder has the default icon back");
  });

  it("says why a run couldn't go at all", () => {
    const s = runSummary(run({ changed: 0, done: 0, error: "that skin isn't available any more" }), "Coral");
    expect(s).toEqual({ title: "Couldn't apply the skin: that skin isn't available any more", detail: null, tone: "warn" });
  });

  it("offers what comes next only when it can be done", () => {
    const stopped = run({ stopped: true, done: 12, changed: 12 });
    expect([canCarryOn(stopped), canRetry(stopped), canUndo(stopped)]).toEqual([true, false, true]);
    const failed = run({ changed: 22, failed: 3 });
    expect([canCarryOn(failed), canRetry(failed), canUndo(failed)]).toEqual([false, true, true]);
    const going = run({ running: true, done: 3 });
    expect([canCarryOn(going), canRetry(going), canUndo(going)]).toEqual([false, false, false]);
    // A revert is never undone, and a stopped run with nothing left has nothing to carry on with.
    expect(canUndo(run({ kind: "revert" }))).toBe(false);
    expect(canCarryOn(run({ stopped: true }))).toBe(false);
    expect(canCarryOn(run({ stopped: true, done: 12, changed: 12, total: 300, counted: false }))).toBe(true);
  });

  it("toasts the whole thing in one line", () => {
    expect(runToast(run({}), "Projects", "Coral")).toBe("Projects and 24 folders inside it now wear Coral");
    expect(runToast(run({ changed: 24, failed: 1 }), "Projects", "Coral")).toBe("24 of 25 folders now wear Coral. 1 couldn't be changed.");
  });

  it("says in a notification how a run ended, and nothing for one that was stopped", () => {
    expect(runNotice(run({ changed: 48_210, done: 48_210, total: 48_210 }), "Dune")).toEqual({ title: "Dune is on 48,210 folders in Projects", body: "" });
    expect(runNotice(run({ changed: 48_198, failed: 12, done: 48_210, total: 48_210 }), "Dune")).toEqual({
      title: "Dune is on 48,198 folders in Projects",
      body: "12 couldn't be changed.",
    });
    expect(runNotice(run({}), null)?.title).toBe("The skin is on 25 folders in Projects");
    expect(runNotice(run({ kind: "revert", changed: 14, skipped: 11 }), null)?.title).toBe("14 folders in Projects have the default icon back");
    expect(runNotice(run({ kind: "revert", changed: 0, skipped: 25 }), null)?.title).toBe("No folder in Projects had a custom icon");
    expect(runNotice(run({ stopped: true, done: 3 }), "Dune")).toBeNull();
    expect(runNotice(run({ running: true }), "Dune")).toBeNull();
    expect(runNotice(run({ error: "no" }), "Dune")).toBeNull();
  });
});
