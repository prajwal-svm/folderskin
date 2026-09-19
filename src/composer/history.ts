/**
 * Undo and redo for the composer. A reducer over immutable documents, so it is unit-tested
 * without React.
 *
 * Two kinds of change keep the history tidy:
 * - A drag on the canvas is live (`preview`) until the pointer lets go (`settle`), which makes
 *   the whole drag one step.
 * - A change with a `key` (a slider for one layer's opacity, typing into one text) merges into
 *   the step before it when that step had the same key and came less than `MERGE_MS` earlier,
 *   so sliding or typing is one step too.
 */
import type { Doc } from "./doc";

export const LIMIT = 100;
export const MERGE_MS = 1000;

export type History = {
  past: Doc[];
  present: Doc;
  future: Doc[];
  /** The document a live change started from, while it's under way. */
  pending: Doc | null;
  /** The key and time of the last step, for merging. */
  last: { key: string; at: number } | null;
};

export type HistoryAction =
  | { type: "commit"; doc: Doc; key?: string; at?: number }
  | { type: "preview"; doc: Doc }
  | { type: "settle" }
  | { type: "undo" }
  | { type: "redo" }
  | { type: "reset"; doc: Doc };

export const startHistory = (doc: Doc): History => ({ past: [], present: doc, future: [], pending: null, last: null });

const pushed = (past: Doc[], doc: Doc) => (past.length >= LIMIT ? [...past.slice(past.length - LIMIT + 1), doc] : [...past, doc]);

/** Ends a live change: one step from where it started, if it changed anything. */
function settle(h: History): History {
  if (!h.pending) return h;
  if (h.pending === h.present) return { ...h, pending: null };
  return { ...h, past: pushed(h.past, h.pending), future: [], pending: null, last: null };
}

export function historyReducer(h: History, action: HistoryAction): History {
  switch (action.type) {
    case "commit": {
      const base = settle(h);
      if (action.doc === base.present) return base;
      const at = action.at ?? Date.now();
      const merge = action.key !== undefined && base.last?.key === action.key && at - base.last.at < MERGE_MS && base.past.length > 0;
      return {
        past: merge ? base.past : pushed(base.past, base.present),
        present: action.doc,
        future: [],
        pending: null,
        last: action.key !== undefined ? { key: action.key, at } : null,
      };
    }
    case "preview":
      return { ...h, present: action.doc, pending: h.pending ?? h.present };
    case "settle":
      return settle(h);
    case "undo": {
      const s = settle(h);
      if (s.past.length === 0) return s;
      const prev = s.past[s.past.length - 1];
      return { past: s.past.slice(0, -1), present: prev, future: [s.present, ...s.future], pending: null, last: null };
    }
    case "redo": {
      const s = settle(h);
      if (s.future.length === 0) return s;
      const [next, ...rest] = s.future;
      return { past: pushed(s.past, s.present), present: next, future: rest, pending: null, last: null };
    }
    case "reset":
      return startHistory(action.doc);
  }
}

export const canUndo = (h: History) => h.past.length > 0 || (h.pending !== null && h.pending !== h.present);
export const canRedo = (h: History) => h.future.length > 0;
