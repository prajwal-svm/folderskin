import { describe, expect, it } from "vitest";
import { applyLabel, folders, formatBytes, mergeRuns, runSummary, runToast, tooMany, type TreeRun } from "./tree";

const run = (patch: Partial<TreeRun>): TreeRun => ({
  kind: "apply",
  total: 25,
  changed: [],
  failed: [],
  skipped: 0,
  remaining: [],
  stopped: false,
  ...patch,
});
const paths = (count: number, from = 0) => Array.from({ length: count }, (_, i) => `/p/${from + i}`);

describe("runs over a folder and its subfolders", () => {
  it("counts folders the way people say them", () => {
    expect(folders(1)).toBe("1 folder");
    expect(folders(1234)).toBe("1,234 folders");
    expect(applyLabel(24)).toBe("Apply to 25 folders");
    expect(applyLabel(1)).toBe("Apply to 2 folders");
  });

  it("says why a tree is too big to include", () => {
    expect(tooMany("folders")).toBe("Too many folders (over 5,000)");
    expect(tooMany("subfolders")).toBe("Too many subfolders (over 5,000)");
  });

  it("writes sizes people can read", () => {
    expect(formatBytes(900)).toBe("900 bytes");
    expect(formatBytes(3_718_461)).toBe("3.7 MB");
    expect(formatBytes(92_961_525)).toBe("93 MB");
    expect(formatBytes(1_250_000_000)).toBe("1.3 GB");
  });

  it("sums up an apply that reached every folder", () => {
    expect(runSummary(run({ changed: paths(25) }), "Coral")).toEqual({ title: "25 folders now wear Coral", detail: null, tone: "ok" });
    expect(runSummary(run({ total: 1, changed: paths(1) }), "Coral").title).toBe("1 folder now wears Coral");
  });

  it("says how many couldn't be changed", () => {
    const s = runSummary(run({ changed: paths(22), failed: [{ path: "/x", name: "x", reason: "no" }, { path: "/y", name: "y", reason: "no" }, { path: "/z", name: "z", reason: "no" }] }), "Coral");
    expect(s).toEqual({ title: "22 of 25 folders now wear Coral", detail: "3 couldn't be changed.", tone: "warn" });
  });

  it("says where a stopped run got to", () => {
    const s = runSummary(run({ changed: paths(12), remaining: paths(13, 12), stopped: true }), "Coral");
    expect(s.title).toBe("Stopped after 12 folders");
    expect(s.detail).toBe("Those 12 wear Coral. 13 weren't reached.");
    expect(runSummary(run({ stopped: true, remaining: paths(25) }), "Coral")).toEqual({ title: "Stopped before any folder was changed", detail: null, tone: "warn" });
  });

  it("sums up a revert, with the folders it left alone", () => {
    expect(runSummary(run({ kind: "revert", changed: paths(14), skipped: 11 }), null)).toEqual({
      title: "14 folders have the default icon back",
      detail: "11 folders had no icon of their own.",
      tone: "ok",
    });
    expect(runSummary(run({ kind: "revert", skipped: 25 }), null).title).toBe("No folder here has a custom icon");
    expect(runSummary(run({ kind: "revert", changed: paths(1) }), null).title).toBe("1 folder has the default icon back");
  });

  it("adds a carried-on run to the stopped one", () => {
    const first = run({ changed: paths(12), remaining: paths(13, 12), stopped: true });
    const next = run({ total: 13, changed: paths(12, 12), failed: [{ path: "/p/24", name: "24", reason: "no" }] });
    const merged = mergeRuns(first, next);
    expect(merged.total).toBe(25);
    expect(merged.changed).toHaveLength(24);
    expect(merged.failed.map((f) => f.path)).toEqual(["/p/24"]);
    expect(merged.stopped).toBe(false);
    expect(merged.remaining).toEqual([]);
  });

  it("carries on taking icons off the way the stopped run did", () => {
    // Taking the icons off a tree left the folders without one alone, and carrying on does too.
    const first = run({ kind: "revert", leavesPlain: true, changed: paths(3), skipped: 5, remaining: paths(17, 8), stopped: true });
    const next = run({ kind: "revert", leavesPlain: true, total: 17, changed: paths(2, 8), skipped: 15 });
    const merged = mergeRuns(first, next);
    expect(merged.leavesPlain).toBe(true);
    expect(merged.skipped).toBe(20);
    expect(merged.changed).toHaveLength(5);
  });

  it("forgets a failure that worked the second time", () => {
    const first = run({ changed: paths(23), failed: [{ path: "/a", name: "a", reason: "no" }, { path: "/b", name: "b", reason: "no" }] });
    const retry = run({ total: 2, changed: ["/a"], failed: [{ path: "/b", name: "b", reason: "still no" }] });
    const merged = mergeRuns(first, retry);
    expect(merged.changed).toHaveLength(24);
    expect(merged.failed).toEqual([{ path: "/b", name: "b", reason: "still no" }]);
  });

  it("toasts the whole thing in one line", () => {
    expect(runToast(run({ changed: paths(25) }), "Projects", "Coral")).toBe("Projects and 24 folders inside it now wear Coral");
    expect(runToast(run({ changed: paths(22), failed: [{ path: "/x", name: "x", reason: "no" }] }), "Projects", "Coral")).toBe(
      "22 of 25 folders now wear Coral. 1 couldn't be changed.",
    );
  });
});
