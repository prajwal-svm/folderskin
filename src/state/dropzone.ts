/**
 * The drop zone's state machine. Pure and synchronous: the React layer dispatches
 * actions around the async Tauri calls, so every transition here is unit-testable.
 */

export type Folder = { path: string; name: string };

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
  /** True while something is being dragged over the window. */
  hover: boolean;
};

export type Action =
  | { type: "drag"; hover: boolean }
  | { type: "folderDropped"; folder: Folder }
  | { type: "skinSelected"; skinId: string }
  | { type: "applyStarted" }
  | { type: "applySucceeded" }
  | { type: "applyFailed"; message: string }
  | { type: "revertStarted" }
  | { type: "revertSucceeded" }
  | { type: "revertFailed"; message: string }
  | { type: "invalidDrop"; message: string }
  | { type: "clearError" };

export const initialState: State = {
  phase: "idle",
  folder: null,
  skinId: null,
  appliedSkinId: null,
  inFlightSkinId: null,
  error: null,
  hover: false,
};

function actionablePhase(state: State): Phase {
  if (!state.folder) return "idle";
  if (state.appliedSkinId && state.appliedSkinId === state.skinId) return "applied";
  return state.skinId ? "ready" : "folder";
}

export function reduce(state: State, action: Action): State {
  switch (action.type) {
    case "drag":
      return { ...state, hover: action.hover };

    case "folderDropped": {
      const next = { ...state, folder: action.folder, appliedSkinId: null, inFlightSkinId: null, error: null, hover: false };
      return { ...next, phase: actionablePhase(next) };
    }

    case "skinSelected": {
      const next = { ...state, skinId: action.skinId, error: null };
      if (state.phase === "applying" || state.phase === "reverting") return next;
      return { ...next, phase: actionablePhase(next) };
    }

    case "applyStarted":
      if (!state.folder || !state.skinId || state.phase === "applying" || state.phase === "reverting") return state;
      return { ...state, phase: "applying", inFlightSkinId: state.skinId, error: null };

    case "applySucceeded": {
      if (state.phase !== "applying") return state;
      const next = { ...state, appliedSkinId: state.inFlightSkinId, inFlightSkinId: null };
      return { ...next, phase: actionablePhase(next) };
    }

    case "applyFailed": {
      if (state.phase !== "applying") return state;
      const next = { ...state, inFlightSkinId: null, error: action.message };
      return { ...next, phase: actionablePhase(next) };
    }

    case "revertStarted":
      if (state.phase !== "applied") return state;
      return { ...state, phase: "reverting", error: null };

    case "revertSucceeded": {
      if (state.phase !== "reverting") return state;
      const next = { ...state, appliedSkinId: null };
      return { ...next, phase: actionablePhase(next) };
    }

    case "revertFailed":
      if (state.phase !== "reverting") return state;
      return { ...state, phase: "applied", error: action.message };

    case "invalidDrop":
      return { ...state, hover: false, error: action.message };

    case "clearError":
      return { ...state, error: null };
  }
}

export type ButtonLabel = "Apply skin" | "Applying…" | "Applied" | "Reverting…";

/** What the action button says, or null when there is nothing to press yet. */
export function buttonLabel(state: State): ButtonLabel | null {
  switch (state.phase) {
    case "ready":
      return "Apply skin";
    case "applying":
      return "Applying…";
    case "applied":
      return "Applied";
    case "reverting":
      return "Reverting…";
    default:
      return null;
  }
}

/** The zone shows the glowing gradient border once a skin is chosen for a folder. */
export function hasGlow(state: State): boolean {
  return state.phase === "ready" || state.phase === "applying" || state.phase === "applied" || state.phase === "reverting";
}
