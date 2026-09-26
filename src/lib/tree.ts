/**
 * A skin for a folder and every folder inside it: the words the folder panel uses for such a
 * run, and how a run that carries on after a stop, or tries the failures again, adds to the one
 * before it. Pure but for the language they speak in (src/i18n), so they're tested without the app.
 */

import { clip } from "./names";
import { t } from "../i18n";
import { formatNumber } from "../i18n/format";

export { formatBytes } from "../i18n/format";

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

/**
 * A run, and which way it went. A revert that left alone the folders with no icon of their own
 * (`leavesPlain`), as taking the icons off a tree does, leaves them alone again when it's carried
 * on; one that undid an apply takes off exactly what that put on.
 */
export type TreeRun = TreeRunResult & { kind: "apply" | "revert"; leavesPlain?: boolean };

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
export const tooMany = (what: "folders" | "subfolders") => t(`folder.run.tooMany.${what}`, { max: MAX_TREE });

/** Runs over more folders than this ask first; smaller ones start straight away. */
export const CONFIRM_ABOVE = 10;

/** "1 folder", "1,234 folders". */
export const folders = (count: number) => t("folder.run.folders", { count });

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
    leavesPlain: next.leavesPlain,
    total: prev.total,
    changed,
    failed: [...prev.failed.filter((f) => !retried.has(f.path)), ...next.failed],
    skipped: prev.skipped + next.skipped,
    remaining: next.remaining,
    stopped: next.stopped,
  };
}

/** The Apply button with subfolders included: the folder and the ones inside it. */
export const applyLabel = (inside: number) => t("folder.run.applyTo", { count: inside + 1 });

/** Two whole sentences one after the other, as the language puts them side by side. */
const both = (first: string, second: string) => t("common.twoSentences", { first, second });

/** The two lines a finished run shows: what happened, then anything worth knowing. */
export function runSummary(run: TreeRun, skinName: string | null): { title: string; detail: string | null; tone: "ok" | "warn" } {
  const changed = run.changed.length;
  const failed = run.failed.length;
  const skin = skinName ? clip(skinName) : t("folder.run.theSkin");
  const couldnt = t("folder.run.couldntChange", { count: failed });
  if (run.kind === "apply") {
    if (run.stopped) {
      if (changed === 0) return { title: t("folder.run.stoppedBeforeAny"), detail: null, tone: "warn" };
      return {
        title: t("folder.run.stoppedAfter", { count: changed }),
        detail: both(t("folder.run.thoseWear", { count: changed, skin }), t("folder.run.notReached", { count: run.remaining.length })),
        tone: "warn",
      };
    }
    if (failed === 0) return { title: t("folder.run.allWear", { count: changed, skin }), detail: null, tone: "ok" };
    return {
      title: t("folder.run.someWear", { count: changed, total: formatNumber(run.total), skin }),
      detail: couldnt,
      tone: "warn",
    };
  }
  if (run.stopped) {
    return {
      title: t("folder.run.stoppedAfter", { count: changed }),
      detail: t("folder.run.thoseBack", { count: changed }),
      tone: "warn",
    };
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
  const inside = run.changed.length - 1;
  if (run.kind === "apply" && !run.stopped && run.failed.length === 0 && inside > 0) {
    return t("folder.run.toastAllWear", { folder: clip(folderName), count: inside, skin: clip(skinName) });
  }
  return s.detail ? t("folder.run.toast", { title: s.title, detail: s.detail }) : s.title;
}
