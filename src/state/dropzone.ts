/**
 * The drop zone's state machine. Pure and synchronous: the React layer dispatches
 * actions around the async Tauri calls, so every transition here is unit-testable.
 */
import type { Subfolders, TreeProgress, TreeRun } from "../lib/tree";

export type Folder = { path: string; name: string };

/**
 * The folders inside the chosen one that a run over the tree takes when they aren't all of them,
 * as ticked in the chooser (components/SubfolderChooser.tsx).
 */
export type SubfolderChoice = {
  /** The folder itself, as a run names it. */
  root: string;
  /** The folders inside it that were ticked, nearest first as a run goes through them. */
  paths: string[];
  /** How many folders were inside it in all. */
  total: number;
};

/** What is being dragged over the window, guessed from its path before it lands. */
export type DragInfo = { kind: "folder" | "image"; name: string };

export type Phase = "idle" | "folder" | "ready" | "applying" | "applied" | "reverting";

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
  /** The folders inside the chosen one: null until they've been counted. */
  subfolders: Subfolders | null;
  /** Apply and revert reach the folders inside the chosen one too. */
  includeSubfolders: boolean;
  /** Which folders inside it they reach when not all of them. Kept while the same folder is chosen. */
  chosen: SubfolderChoice | null;
  /** How far an apply or revert over the folder and its subfolders has got. */
  progress: TreeProgress | null;
  /** What the last run over the folder and its subfolders did, until the skin or folder changes. */
  run: TreeRun | null;
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
  /** The chooser was closed with Done: `chosen` is null when every one of the `total` folders
   *  inside `path` was ticked. */
  | { type: "subfoldersChosen"; path: string; total: number; chosen: SubfolderChoice | null }
  | { type: "applyStarted" }
  | { type: "treeProgress"; progress: TreeProgress }
  | { type: "applySucceeded"; run?: TreeRun }
  | { type: "applyFailed"; message: string }
  | { type: "revertStarted" }
  | { type: "revertSucceeded"; run?: TreeRun }
  | { type: "revertFailed"; message: string }
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
  progress: null,
  run: null,
};

const busy = (state: State) => state.phase === "applying" || state.phase === "reverting";

/** How many folders inside the chosen one a run over the tree takes: none unless they're included, then every one or the ones chosen. */
export function insideCount(state: State): number {
  if (!state.includeSubfolders || !state.subfolders) return 0;
  return state.chosen ? state.chosen.paths.length : state.subfolders.count;
}

/** What a run over the tree names as `only`: the folder and the ones chosen inside it, or null for the whole tree. */
export function treeOnly(state: State): string[] | null {
  return state.includeSubfolders && state.chosen ? [state.chosen.root, ...state.chosen.paths] : null;
}

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
      // folder picked again keeps the folders chosen inside it.
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
        progress: null,
        run: null,
      };
      return { ...next, phase: actionablePhase(next) };
    }

    case "subfoldersCounted": {
      if (state.folder?.path !== action.path) return state;
      const none = !action.subfolders || action.subfolders.count === 0 || action.subfolders.more;
      return { ...state, subfolders: action.subfolders, includeSubfolders: none ? false : state.includeSubfolders };
    }

    case "includeSubfolders": {
      if (busy(state)) return state;
      const s = state.subfolders;
      if (action.on && (!s || s.count === 0 || s.more)) return state;
      return { ...state, includeSubfolders: action.on };
    }

    // Done in the chooser. Every folder ticked is the whole tree again, as it was before anything
    // was chosen, and none ticked is the folder on its own.
    case "subfoldersChosen": {
      if (busy(state) || state.folder?.path !== action.path) return state;
      const some = action.chosen !== null && action.chosen.paths.length > 0;
      return {
        ...state,
        subfolders: { count: action.total, more: false },
        chosen: some ? action.chosen : null,
        includeSubfolders: some || (action.chosen === null && action.total > 0),
      };
    }

    case "treeProgress":
      return busy(state) ? { ...state, progress: action.progress } : state;

    case "runDismissed":
      return state.run ? { ...state, run: null } : state;

    case "arrived":
      return state.arriving ? { ...state, arriving: false } : state;

    case "skinSelected": {
      const next = { ...state, skinId: action.skinId, error: null, arriving: false, run: state.skinId === action.skinId ? state.run : null };
      if (state.phase === "applying" || state.phase === "reverting") return next;
      return { ...next, phase: actionablePhase(next) };
    }

    case "skinCleared": {
      if (state.phase === "applying" || state.phase === "reverting") return state;
      const next = { ...state, skinId: null, error: null, arriving: false };
      return { ...next, phase: actionablePhase(next) };
    }

    case "applyStarted":
      if (!state.folder || !state.skinId || busy(state)) return state;
      return { ...state, phase: "applying", inFlightSkinId: state.skinId, error: null, arriving: false, progress: null, run: null };

    case "applySucceeded": {
      if (state.phase !== "applying") return state;
      // A run that stopped before changing anything leaves the folder as it was.
      const changedNothing = action.run !== undefined && action.run.changed.length === 0;
      const next = {
        ...state,
        appliedSkinId: changedNothing ? state.appliedSkinId : state.inFlightSkinId,
        inFlightSkinId: null,
        progress: null,
        run: action.run ?? null,
      };
      return { ...next, phase: actionablePhase(next) };
    }

    case "applyFailed": {
      if (state.phase !== "applying") return state;
      const next = { ...state, inFlightSkinId: null, error: action.message, progress: null };
      return { ...next, phase: actionablePhase(next) };
    }

    // A revert takes off the skin just applied, or an icon the folder already had while it's on
    // show (on its own, or while it waits before a skin goes on).
    case "revertStarted":
      if (state.phase !== "applied" && state.phase !== "folder" && !(state.phase === "ready" && state.arriving)) return state;
      return { ...state, phase: "reverting", error: null, progress: null };

    case "revertSucceeded": {
      // The skin is put down too, so the folder is seen wearing its default icon again
      // instead of jumping straight back into a preview of the skin just removed.
      if (state.phase !== "reverting") return state;
      const next = { ...state, appliedSkinId: null, skinId: null, arriving: false, progress: null, run: action.run ?? null };
      return { ...next, phase: actionablePhase(next) };
    }

    case "revertFailed":
      if (state.phase !== "reverting") return state;
      return { ...state, phase: actionablePhase(state), error: action.message, progress: null };

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
