/**
 * The drop zone's state machine. Pure and synchronous: the React layer dispatches
 * actions around the async Tauri calls, so every transition here is unit-testable.
 */
import type { Subfolders } from "../lib/tree";
import { isNothing, type Choice } from "../lib/folderChoice";

export type Folder = { path: string; name: string };

/**
 * The folders inside the chosen one that a run over the tree takes when they aren't all of them,
 * as ticked in the chooser (components/SubfolderChooser.tsx), and how many that is.
 */
export type SubfolderChoice = {
  /** The rules the chooser left (lib/folderChoice.ts). */
  choice: Choice;
  /** How many folders inside it they take, of how many there are, as counted so far: final once `done`. */
  count: number;
  total: number;
  done: boolean;
};

/** What is being dragged over the window, guessed from its path before it lands. */
export type DragInfo = { kind: "folder" | "image"; name: string };

export type Phase = "idle" | "folder" | "ready" | "applying" | "applied" | "reverting";

/** A run over a tree as the folder panel needs to know of it (the whole of it is lib/tree.ts `TreeRun`). */
export type RunInfo = {
  id: number;
  kind: "apply" | "revert";
  /** The skin an apply puts on. */
  skinId: string | null;
  changed: number;
};

export type State = {
  phase: Phase;
  folder: Folder | null;
  /** The skin currently selected in the gallery. */
  skinId: string | null;
  /** The skin the folder currently wears (null when it wears the OS default). */
  appliedSkinId: string | null;
  /** The skin an in-flight apply is writing. */
  inFlightSkinId: string | null;
  error: string | null;
  /** What is being dragged over the window right now, if anything. */
  drag: DragInfo | null;
  /**
   * A folder that replaced another is showing its own icon for a moment before the selected skin
   * goes on, so the switch to it can be seen.
   */
  arriving: boolean;
  /** The folders inside the chosen one, as counted so far: null until the count has begun. */
  subfolders: Subfolders | null;
  /** Apply and revert reach the folders inside the chosen one too. */
  includeSubfolders: boolean;
  /** Which folders inside it they reach when not all of them. Kept while the same folder is chosen. */
  chosen: SubfolderChoice | null;
  /**
   * The run over the folder and its subfolders the panel shows (state/treeRun.ts has it): going
   * on, or ended with its summary showing until it's put away or the skin or folder changes.
   */
  runId: number | null;
};

export type Action =
  | { type: "drag"; info: DragInfo | null }
  | { type: "folderDropped"; folder: Folder }
  | { type: "folderCleared" }
  | { type: "arrived" }
  | { type: "skinSelected"; skinId: string }
  | { type: "skinCleared" }
  | { type: "subfoldersCounted"; path: string; subfolders: Subfolders | null }
  | { type: "includeSubfolders"; on: boolean }
  /** The chooser was closed with Done: `chosen` is null when every folder inside `path` was ticked. */
  | { type: "subfoldersChosen"; path: string; chosen: SubfolderChoice | null }
  /** The folders the choice takes, counted further. */
  | { type: "choiceCounted"; path: string; count: number; total: number; done: boolean }
  | { type: "applyStarted" }
  | { type: "applySucceeded" }
  | { type: "applyFailed"; message: string }
  | { type: "revertStarted" }
  | { type: "revertSucceeded" }
  | { type: "revertFailed"; message: string }
  /** A run over the tree of the folder at `path` is going: started here, or come back to. */
  | { type: "treeRunning"; path: string; run: RunInfo }
  /** The run the panel follows has ended: `error` when it couldn't go at all. */
  | { type: "treeEnded"; run: RunInfo; error: string | null }
  /** Back at the folder of a run that has ended, from the run indicator: its summary shows again. */
  | { type: "treeShown"; path: string; run: RunInfo }
  /** A run over the tree can't start now, and why. */
  | { type: "treeRefused"; message: string }
  | { type: "runDismissed" }
  | { type: "invalidDrop"; message: string }
  | { type: "clearError" };

export const initialState: State = {
  phase: "idle",
  folder: null,
  skinId: null,
  appliedSkinId: null,
  inFlightSkinId: null,
  error: null,
  drag: null,
  arriving: false,
  subfolders: null,
  includeSubfolders: false,
  chosen: null,
  runId: null,
};

const busy = (state: State) => state.phase === "applying" || state.phase === "reverting";

/** How many folders inside the chosen one a run over the tree takes: none unless they're included, then every one or the ones chosen, as counted so far. */
export function insideCount(state: State): number {
  if (!state.includeSubfolders || !state.subfolders) return 0;
  return state.chosen ? state.chosen.count : state.subfolders.count;
}

/** Whether `insideCount` is final: the folders inside have all been counted. */
export function insideCounted(state: State): boolean {
  if (!state.subfolders) return false;
  return state.chosen ? state.chosen.done : state.subfolders.done;
}

/** Whether Apply and revert are a run over the tree: subfolders are included, and there are some, or may be. */
export function isTree(state: State): boolean {
  return state.includeSubfolders && state.subfolders !== null && (insideCount(state) > 0 || !insideCounted(state));
}

/** Which folders inside a run over the tree takes: the choice, or null for every one. */
export function treeChoice(state: State): Choice | null {
  return state.includeSubfolders && state.chosen ? state.chosen.choice : null;
}

/** There are no folders inside, as far as the count knows: it's done and found none. */
const noneInside = (s: Subfolders | null) => !s || (s.done && s.count === 0);

function actionablePhase(state: State): Phase {
  if (!state.folder) return "idle";
  if (state.appliedSkinId && state.appliedSkinId === state.skinId) return "applied";
  return state.skinId ? "ready" : "folder";
}

export function reduce(state: State, action: Action): State {
  switch (action.type) {
    case "drag":
      return { ...state, drag: action.info };

    case "folderDropped": {
      const arriving = state.folder !== null && state.skinId !== null;
      // A new folder starts with only itself in play: its subfolders are counted afresh and
      // including them is chosen again, so a whole tree is never changed by accident. The same
      // folder picked again keeps the folders chosen inside it. A run over the folder before goes
      // on without it.
      const next = {
        ...state,
        folder: action.folder,
        appliedSkinId: null,
        inFlightSkinId: null,
        error: null,
        drag: null,
        arriving,
        subfolders: null,
        includeSubfolders: false,
        chosen: state.folder?.path === action.folder.path ? state.chosen : null,
        runId: null,
      };
      return { ...next, phase: actionablePhase(next) };
    }

    case "subfoldersCounted": {
      if (state.folder?.path !== action.path) return state;
      return { ...state, subfolders: action.subfolders, includeSubfolders: noneInside(action.subfolders) ? false : state.includeSubfolders };
    }

    // On while the folders inside are still being counted too: the count never holds anything up.
    case "includeSubfolders": {
      if (busy(state)) return state;
      if (action.on && noneInside(state.subfolders)) return state;
      return { ...state, includeSubfolders: action.on };
    }

    // Done in the chooser. Every folder ticked is the whole tree again, as it was before anything
    // was chosen, and none ticked is the folder on its own.
    case "subfoldersChosen": {
      if (busy(state) || state.folder?.path !== action.path) return state;
      const chosen = action.chosen;
      const none = chosen !== null && (isNothing(chosen.choice) || (chosen.done && chosen.count === 0));
      return { ...state, chosen: none ? null : chosen, includeSubfolders: !none && !noneInside(state.subfolders) };
    }

    case "choiceCounted": {
      const chosen = state.chosen;
      if (!chosen || state.folder?.path !== action.path) return state;
      const { count, total, done } = action;
      if (chosen.count === count && chosen.total === total && chosen.done === done) return state;
      return { ...state, chosen: { ...chosen, count, total, done } };
    }

    case "treeRunning": {
      if (state.folder?.path !== action.path) return state;
      const { run } = action;
      const apply = run.kind === "apply";
      return {
        ...state,
        phase: apply ? "applying" : "reverting",
        skinId: apply ? run.skinId : state.skinId,
        inFlightSkinId: apply ? run.skinId : null,
        includeSubfolders: true,
        runId: run.id,
        error: null,
        arriving: false,
      };
    }

    case "treeEnded": {
      const { run, error } = action;
      if (!busy(state) || state.runId !== run.id) return state;
      if (error !== null) {
        const next = { ...state, inFlightSkinId: null, error, runId: null };
        return { ...next, phase: actionablePhase(next) };
      }
      if (run.kind === "apply") {
        // A run that stopped before changing anything leaves the folder as it was.
        const next = { ...state, appliedSkinId: run.changed > 0 ? state.inFlightSkinId : state.appliedSkinId, inFlightSkinId: null };
        return { ...next, phase: actionablePhase(next) };
      }
      // The skin is put down too, as after taking a single folder's icon off.
      const next = { ...state, appliedSkinId: null, skinId: null, arriving: false };
      return { ...next, phase: actionablePhase(next) };
    }

    case "treeShown": {
      if (busy(state) || state.folder?.path !== action.path) return state;
      const { run } = action;
      const worn = run.kind === "apply" && run.changed > 0 ? run.skinId : null;
      const next = {
        ...state,
        skinId: worn ?? state.skinId,
        appliedSkinId: worn,
        includeSubfolders: true,
        runId: run.id,
        arriving: false,
        error: null,
      };
      return { ...next, phase: actionablePhase(next) };
    }

    case "treeRefused":
      return { ...state, error: action.message };

    case "runDismissed":
      return state.runId !== null && !busy(state) ? { ...state, runId: null } : state;

    case "arrived":
      return state.arriving ? { ...state, arriving: false } : state;

    case "skinSelected": {
      // Another skin puts the last run's summary away, but not the run that's going.
      const runId = busy(state) || state.skinId === action.skinId ? state.runId : null;
      const next = { ...state, skinId: action.skinId, error: null, arriving: false, runId };
      if (busy(state)) return next;
      return { ...next, phase: actionablePhase(next) };
    }

    case "skinCleared": {
      if (busy(state)) return state;
      const next = { ...state, skinId: null, error: null, arriving: false };
      return { ...next, phase: actionablePhase(next) };
    }

    case "applyStarted":
      if (!state.folder || !state.skinId || busy(state)) return state;
      return { ...state, phase: "applying", inFlightSkinId: state.skinId, error: null, arriving: false, runId: null };

    case "applySucceeded": {
      if (state.phase !== "applying" || state.runId !== null) return state;
      const next = { ...state, appliedSkinId: state.inFlightSkinId, inFlightSkinId: null };
      return { ...next, phase: actionablePhase(next) };
    }

    case "applyFailed": {
      if (state.phase !== "applying" || state.runId !== null) return state;
      const next = { ...state, inFlightSkinId: null, error: action.message };
      return { ...next, phase: actionablePhase(next) };
    }

    // A revert takes off the skin just applied, or an icon the folder already had while it's on
    // show (on its own, or while it waits before a skin goes on).
    case "revertStarted":
      if (state.phase !== "applied" && state.phase !== "folder" && !(state.phase === "ready" && state.arriving)) return state;
      return { ...state, phase: "reverting", error: null, runId: null };

    case "revertSucceeded": {
      // The skin is put down too, so the folder is seen wearing its default icon again
      // instead of jumping straight back into a preview of the skin just removed.
      if (state.phase !== "reverting" || state.runId !== null) return state;
      const next = { ...state, appliedSkinId: null, skinId: null, arriving: false };
      return { ...next, phase: actionablePhase(next) };
    }

    case "revertFailed":
      if (state.phase !== "reverting" || state.runId !== null) return state;
      return { ...state, phase: actionablePhase(state), error: action.message };

    case "invalidDrop":
      return { ...state, drag: null, error: action.message };

    // No folder any more (the AI chat stopped using one); the skin picked stays picked.
    case "folderCleared":
      if (busy(state)) return state;
      return { ...initialState, skinId: state.skinId };

    case "clearError":
      return { ...state, error: null };
  }
}
