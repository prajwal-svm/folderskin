/**
 * A skin for a folder and every folder inside it: what a run over the tree looks like as the app
 * tells of it (src-tauri/src/tree.rs), and the words the app uses for one. Pure but for the
 * language they speak in (src/i18n), so they're tested without the app.
 */

import { clip } from "./names";
import { t } from "../i18n";
import { formatNumber } from "../i18n/format";
import { explain } from "./sentences";

export { formatBytes } from "../i18n/format";

/** A folder a run couldn't change, and why, in a sentence. */
export type TreeFailure = { path: string; name: string; reason: string };

/**
 * A run over a folder and the folders inside it, going or ended, as `tree-run` tells of it. `done`
 * of `total` are finished, `total` being the folders the run takes found so far, and every one of
 * them once `counted`. `changed`, `failed` and `skipped` (a revert leaving a folder with no icon of
 * its own alone) add up to `done`. `failures` are the first of the folders that failed, and `error`
 * why the run couldn't go at all.
 */
export type TreeRun = {
  id: number;
  kind: "apply" | "revert";
  /** A revert that takes off exactly what an apply put on. */
  undoing: boolean;
  /** A revert that leaves alone the folders with no icon of their own. */
  leaves_plain: boolean;
  /** The folder as the webview named it (the folder panel's path), and as the run names it. */
  folder: string;
  root: string;
  name: string;
  skin_id: string | null;
  running: boolean;
  stopping: boolean;
  /** It ended with Stop, with folders left to do. */
  stopped: boolean;
  done: number;
  total: number;
  counted: boolean;
  /** The folder just finished, or the folder itself before any. */
  current: string;
  changed: number;
  failed: number;
  skipped: number;
  failures: TreeFailure[];
  error: string | null;
};

/** The latest run, or none: numbered, so the newest wins whatever order they arrive in. */
export type TreeRunEvent = { seq: number; run: TreeRun | null };

/** The folders inside the chosen one as counted so far: all of them once `done`. */
export type Subfolders = { count: number; done: boolean };

/** How far a count of the folders inside one has got, as `subfolder-count` tells of it. */
export type SubfolderCount = Subfolders & { folder: string };

/** One folder as far as the count has got: whether it has come to it, and the folders inside it. */
export type PathCount = { found: boolean; inside: number; done: boolean };

/** The count of the folders inside one, and of the folders inside each folder asked about. */
export type SubfolderCounts = SubfolderCount & { paths: PathCount[] };

/**
 * One folder's own folders, for a column of "Choose subfolders". A folder inside it is `path`,
 * `separator` and its name, and `nested[i]` says whether `names[i]` has folders inside it: null
 * when there wasn't time to look.
 */
export type SubfolderList = { path: string; separator: string; names: string[]; nested: (boolean | null)[] };

/** How far a run has got, for the composer's Apply button. */
export type TreeProgress = { done: number; total: number; counted: boolean };

/** Runs over more folders than this ask first; smaller ones start straight away. */
export const CONFIRM_ABOVE = 10;

/**
 * Runs over more folders than this, or over a tree still being counted, ask in their own words:
 * with the count so far and that the run goes on in the background.
 */
export const BIG_RUN = 5000;

/** Whether a run over `inside` folders and the folder itself is a big one, as `BIG_RUN` says. */
export const isBig = (inside: number, counted: boolean) => !counted || inside + 1 > BIG_RUN;

/** "1 folder", "1,234 folders". */
export const folders = (count: number) => t("folder.run.folders", { count });

/**
 * The Apply button with subfolders included: the folder and the ones inside it, "Apply to 25
 * folders", or while they're still being counted "Apply to 12,401+ folders".
 */
export const applyLabel = (inside: number, counted = true) =>
  counted ? t("folder.run.applyTo", { count: inside + 1 }) : t("folder.run.applyToCounting", { count: inside + 1 });

/**
 * How many folders a run takes, as far as that's known: the run's own count once its walk has
 * found them all, and before that `expected`, what a count that had finished before the run
 * started found, unless the walk has found more since. Null while there's no telling.
 */
export const knownTotal = (run: Pick<TreeRun, "total" | "counted">, expected: number | null = null): number | null =>
  run.counted ? run.total : expected !== null && expected >= run.total ? expected : null;

/** How far a run has got, as its count reads: "3,120 of 12,000", or "3,120 of 12,000+" until it's known how many there are. */
export const doneOf = (run: Pick<TreeRun, "done" | "total" | "counted">, expected: number | null = null) => {
  const total = knownTotal(run, expected);
  return total === null ? t("folder.stage.doneOfMore", { done: run.done, total: run.total }) : t("folder.stage.doneOf", { done: run.done, total });
};

/** How far a run has got, for the composer's Apply button, with what's known of how many folders it takes. */
export function treeProgress(run: TreeRun, expected: number | null): TreeProgress {
  const total = knownTotal(run, expected);
  return { done: run.done, total: total ?? run.total, counted: total !== null };
}

/** How much of a run's bar is filled, from 0 to 1: null until it's known how many folders there are. */
export const shareOf = (run: Pick<TreeRun, "done" | "total" | "counted">, expected: number | null = null): number | null => {
  const total = knownTotal(run, expected);
  return total === null || total === 0 ? null : Math.min(1, run.done / total);
};

/** The folders a stopped run has left, or null when they aren't all found yet. */
export const remaining = (run: TreeRun): number | null => (run.counted ? Math.max(0, run.total - run.done) : null);

/** A stopped run can carry on with the folders it didn't reach. */
export const canCarryOn = (run: TreeRun) => !run.running && run.stopped && run.error === null && remaining(run) !== 0;

/** A run that finished with failures can try them again. */
export const canRetry = (run: TreeRun) => !run.running && !run.stopped && run.error === null && run.failed > 0;

/** An apply that changed something can be undone: exactly those folders go back. */
export const canUndo = (run: TreeRun) => !run.running && run.kind === "apply" && run.error === null && run.changed > 0;

/** What a run that's going is doing, in a few words: "Applying Dune", "Taking Dune off", "Removing custom icons". */
export function runDoing(run: TreeRun, skinName: string | null): string {
  if (run.kind === "apply") return skinName ? t("folder.dock.applying", { skin: clip(skinName) }) : t("folder.dock.applyingSkin");
  if (run.undoing) return skinName ? t("folder.dock.undoing", { skin: clip(skinName) }) : t("folder.dock.undoingSkin");
  return t("folder.dock.removing");
}

/** Two whole sentences one after the other, as the language puts them side by side. */
export const both = (first: string, second: string) => t("common.twoSentences", { first, second });

/** The two lines an ended run shows: what happened, then anything worth knowing. */
export function runSummary(run: TreeRun, skinName: string | null): { title: string; detail: string | null; tone: "ok" | "warn" } {
  const { changed, failed } = run;
  const skin = skinName ? clip(skinName) : t("folder.run.theSkin");
  const couldnt = t("folder.run.couldntChange", { count: failed });
  const left = remaining(run);
  const notReached = left === null ? t("folder.run.notReachedRest") : t("folder.run.notReached", { count: left });
  if (run.error !== null) {
    const reason = explain(run.error);
    return { title: run.kind === "apply" ? t("folder.errors.apply", { reason }) : t("folder.errors.revertTree", { reason }), detail: null, tone: "warn" };
  }
  if (run.kind === "apply") {
    if (run.stopped) {
      if (changed === 0) return { title: t("folder.run.stoppedBeforeAny"), detail: failed > 0 ? couldnt : null, tone: "warn" };
      if (failed === 0) return { title: t("folder.run.stoppedAfter", { count: changed }), detail: both(t("folder.run.thoseWear", { count: changed, skin }), notReached), tone: "warn" };
      // Some it went through couldn't be changed: it stopped after all of those, and says which way each went.
      const wear = both(t("folder.run.ofThemWear", { count: changed, skin }), couldnt);
      return { title: t("folder.run.stoppedAfter", { count: run.done }), detail: both(wear, notReached), tone: "warn" };
    }
    if (failed === 0) return { title: t("folder.run.allWear", { count: changed, skin }), detail: null, tone: "ok" };
    return { title: t("folder.run.someWear", { count: changed, total: formatNumber(run.total), skin }), detail: couldnt, tone: "warn" };
  }
  if (run.stopped) {
    const back = t("folder.run.thoseBack", { count: changed });
    return { title: t("folder.run.stoppedAfter", { count: changed }), detail: failed > 0 ? both(back, couldnt) : back, tone: "warn" };
  }
  const leftAlone = run.skipped > 0 ? t("folder.run.noIconOfTheirOwn", { count: run.skipped }) : null;
  if (failed === 0) {
    return {
      title: changed === 0 ? t("folder.run.noCustomIcon") : t("folder.run.allBack", { count: changed }),
      detail: changed === 0 ? null : leftAlone,
      tone: "ok",
    };
  }
  return {
    title: t("folder.run.someBack", { count: changed, total: formatNumber(changed + failed) }),
    detail: leftAlone ? both(couldnt, leftAlone) : couldnt,
    tone: "warn",
  };
}

/** The toast after a run, for when the folder panel isn't in view (the composer's Save & apply). */
export function runToast(run: TreeRun, folderName: string, skinName: string): string {
  const s = runSummary(run, skinName);
  const inside = run.changed - 1;
  if (run.kind === "apply" && !run.stopped && run.failed === 0 && run.error === null && inside > 0) {
    return t("folder.run.toastAllWear", { folder: clip(folderName), count: inside, skin: clip(skinName) });
  }
  return s.detail ? t("folder.run.toast", { title: s.title, detail: s.detail }) : s.title;
}

/**
 * The system notification for a run that ended while the window was behind others: "Dune is on
 * 48,210 folders in Projects", and "12 couldn't be changed." under it when some couldn't. None for
 * a run that was stopped or never went: someone was there to see that.
 */
export function runNotice(run: TreeRun, skinName: string | null): { title: string; body: string } | null {
  if (run.running || run.stopped || run.error !== null) return null;
  const folder = clip(run.name);
  const body = run.failed > 0 ? t("folder.run.couldntChange", { count: run.failed }) : "";
  if (run.kind === "apply") {
    const title = skinName ? t("folder.run.noticeWear", { skin: clip(skinName), count: run.changed, folder }) : t("folder.run.noticeWearSkin", { count: run.changed, folder });
    return { title, body };
  }
  return { title: run.changed === 0 && run.failed === 0 ? t("folder.run.noticeNone", { folder }) : t("folder.run.noticeBack", { count: run.changed, folder }), body };
}
