/**
 * A skin for a folder and every folder inside it: the words the folder panel uses for such a
 * run, and how a run that carries on after a stop, or tries the failures again, adds to the one
 * before it. Pure, so they're tested without the app.
 */

import { clip } from "./names";

/** A folder a run couldn't change, and why, in a sentence. */
export type TreeFailure = { path: string; name: string; reason: string };

/** What a run over a folder and its subfolders did, as `apply_skin_tree` or `revert_skin_tree` reports it. */
export type TreeRunResult = {
  /** Folders the run set out to change. */
  total: number;
  /** Folders it changed. */
  changed: string[];
  failed: TreeFailure[];
  /** Folders a revert left alone because they had no icon of their own. */
  skipped: number;
  /** Folders a stopped run didn't reach. */
  remaining: string[];
  stopped: boolean;
};

export type TreeRun = TreeRunResult & { kind: "apply" | "revert" };

/** How far a run has got: `done` of `total`, the last folder it finished. */
export type TreeProgress = { done: number; total: number; name: string };

/** The folders inside the chosen one, at most `MAX_TREE` of them. */
export type Subfolders = { count: number; more: boolean };

/** The most folders one run takes on; the Rust side counts no further. */
export const MAX_TREE = 5000;

/**
 * Why the switch can't be turned on for a folder with more than `MAX_TREE` inside: "folders"
 * under the switch's own heading, "subfolders" where the switch stands alone.
 */
export const tooMany = (what: "folders" | "subfolders") => `Too many ${what} (over ${n(MAX_TREE)})`;

/** Runs over more folders than this ask first; smaller ones start straight away. */
export const CONFIRM_ABOVE = 10;

const n = (count: number) => count.toLocaleString("en-US");
export const folders = (count: number) => `${n(count)} ${count === 1 ? "folder" : "folders"}`;
/** "wears" for one, "wear" for more; "has" and "have" likewise. */
const wear = (count: number) => (count === 1 ? "wears" : "wear");
const have = (count: number) => (count === 1 ? "has" : "have");

/**
 * A run after `prev` stopped or left failures: `next` worked through only what was left, so
 * what's changed is both runs' work, what failed is what failed last time, and the total stays
 * the first run's.
 */
export function mergeRuns(prev: TreeRun, next: TreeRun): TreeRun {
  const changed = [...prev.changed, ...next.changed.filter((p) => !prev.changed.includes(p))];
  const retried = new Set([...next.changed, ...next.failed.map((f) => f.path), ...next.remaining]);
  return {
    kind: next.kind,
    total: prev.total,
    changed,
    failed: [...prev.failed.filter((f) => !retried.has(f.path)), ...next.failed],
    skipped: prev.skipped + next.skipped,
    remaining: next.remaining,
    stopped: next.stopped,
  };
}

/** The Apply button with subfolders included: the folder and the ones inside it. */
export const applyLabel = (inside: number) => `Apply to ${folders(inside + 1)}`;

/** A byte count people can read: "3.6 MB", "88 MB", "1.2 GB". */
export function formatBytes(bytes: number): string {
  if (bytes < 1000) return `${bytes} bytes`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = bytes / 1000;
  let i = 0;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  return `${v >= 10 ? Math.round(v) : Math.round(v * 10) / 10} ${units[i]}`;
}

/** The two lines a finished run shows: what happened, then anything worth knowing. */
export function runSummary(run: TreeRun, skinName: string | null): { title: string; detail: string | null; tone: "ok" | "warn" } {
  const changed = run.changed.length;
  const failed = run.failed.length;
  const skin = skinName ? clip(skinName) : "the skin";
  if (run.kind === "apply") {
    if (run.stopped) {
      if (changed === 0) return { title: "Stopped before any folder was changed", detail: null, tone: "warn" };
      return {
        title: `Stopped after ${folders(changed)}`,
        detail: `${changed === 1 ? "That one wears" : `Those ${n(changed)} wear`} ${skin}; ${n(run.remaining.length)} weren't reached.`,
        tone: "warn",
      };
    }
    if (failed === 0) return { title: `${folders(changed)} now ${wear(changed)} ${skin}`, detail: null, tone: "ok" };
    return {
      title: `${n(changed)} of ${folders(run.total)} now ${wear(changed)} ${skin}`,
      detail: `${n(failed)} couldn't be changed.`,
      tone: "warn",
    };
  }
  if (run.stopped) {
    return {
      title: `Stopped after ${folders(changed)}`,
      detail: `${changed === 1 ? "That one has" : `Those ${n(changed)} have`} the default icon back.`,
      tone: "warn",
    };
  }
  const leftAlone = run.skipped > 0 ? `${folders(run.skipped)} had no icon of ${run.skipped === 1 ? "its" : "their"} own.` : null;
  if (failed === 0) {
    return {
      title: changed === 0 ? "No folder here has a custom icon" : `${folders(changed)} ${have(changed)} the default icon back`,
      detail: changed === 0 ? null : leftAlone,
      tone: "ok",
    };
  }
  return {
    title: `${n(changed)} of ${folders(changed + failed)} ${have(changed)} the default icon back`,
    detail: `${n(failed)} couldn't be changed.${leftAlone ? ` ${leftAlone}` : ""}`,
    tone: "warn",
  };
}

/** The toast after a run, for when the folder panel isn't in view (the composer's Save & apply). */
export function runToast(run: TreeRun, folderName: string, skinName: string): string {
  const s = runSummary(run, skinName);
  const inside = run.changed.length - 1;
  if (run.kind === "apply" && !run.stopped && run.failed.length === 0 && inside > 0) {
    return `${clip(folderName)} and ${folders(inside)} inside it now wear ${clip(skinName)}`;
  }
  return s.detail ? `${s.title}. ${s.detail}` : s.title;
}
