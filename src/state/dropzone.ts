/**
 * The drop zone's state machine. Pure and synchronous: the React layer dispatches
 * actions around the async Tauri calls, so every transition here is unit-testable.
 */

export type Folder = { path: string; name: string };

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
};

export type Action =
  | { type: "drag"; info: DragInfo | null }
  | { type: "folderDropped"; folder: Folder }
  | { type: "arrived" }
  | { type: "skinSelected"; skinId: string }
  | { type: "skinCleared" }
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
  drag: null,
  arriving: false,
};

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
      const next = { ...state, folder: action.folder, appliedSkinId: null, inFlightSkinId: null, error: null, drag: null, arriving };
      return { ...next, phase: actionablePhase(next) };
    }

    case "arrived":
      return state.arriving ? { ...state, arriving: false } : state;

    case "skinSelected": {
      const next = { ...state, skinId: action.skinId, error: null, arriving: false };
      if (state.phase === "applying" || state.phase === "reverting") return next;
      return { ...next, phase: actionablePhase(next) };
    }

    case "skinCleared": {
      if (state.phase === "applying" || state.phase === "reverting") return state;
      const next = { ...state, skinId: null, error: null, arriving: false };
      return { ...next, phase: actionablePhase(next) };
    }

    case "applyStarted":
      if (!state.folder || !state.skinId || state.phase === "applying" || state.phase === "reverting") return state;
      return { ...state, phase: "applying", inFlightSkinId: state.skinId, error: null, arriving: false };

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

    // A revert takes off the skin just applied, or an icon the folder already had while it's on
    // show (on its own, or while it waits before a skin goes on).
    case "revertStarted":
      if (state.phase !== "applied" && state.phase !== "folder" && !(state.phase === "ready" && state.arriving)) return state;
      return { ...state, phase: "reverting", error: null };

    case "revertSucceeded": {
      // The skin is put down too, so the folder is seen wearing its default icon again
      // instead of jumping straight back into a preview of the skin just removed.
      if (state.phase !== "reverting") return state;
      const next = { ...state, appliedSkinId: null, skinId: null, arriving: false };
      return { ...next, phase: actionablePhase(next) };
    }

    case "revertFailed":
      if (state.phase !== "reverting") return state;
      return { ...state, phase: actionablePhase(state), error: action.message };

    case "invalidDrop":
      return { ...state, drag: null, error: action.message };

    case "clearError":
      return { ...state, error: null };
  }
}
