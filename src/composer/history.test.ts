import { describe, expect, it } from "vitest";
import { addLayer, emptyDoc, makeEmoji, type Doc } from "./doc";
import { canRedo, canUndo, historyReducer, LIMIT, MERGE_MS, startHistory } from "./history";

const step = (d: Doc) => addLayer(d, makeEmoji("⭐", 0, 0));

describe("undo and redo", () => {
  it("steps back and forward", () => {
    const a = emptyDoc();
    const b = step(a);
    const c = step(b);
    let h = startHistory(a);
    h = historyReducer(h, { type: "commit", doc: b });
    h = historyReducer(h, { type: "commit", doc: c });
    expect(canUndo(h)).toBe(true);
    h = historyReducer(h, { type: "undo" });
    expect(h.present).toBe(b);
    h = historyReducer(h, { type: "undo" });
    expect(h.present).toBe(a);
    expect(canUndo(h)).toBe(false);
    h = historyReducer(h, { type: "redo" });
    expect(h.present).toBe(b);
    expect(canRedo(h)).toBe(true);
    // A new change throws the redo steps away.
    h = historyReducer(h, { type: "commit", doc: step(b) });
    expect(canRedo(h)).toBe(false);
  });

  it("makes a whole drag one step", () => {
    const a = emptyDoc();
    let h = startHistory(a);
    const moves = [step(a), step(step(a)), step(step(step(a)))];
    for (const d of moves) h = historyReducer(h, { type: "preview", doc: d });
    expect(h.present).toBe(moves[2]);
    expect(h.past).toHaveLength(0);
    expect(canUndo(h)).toBe(true);
    h = historyReducer(h, { type: "settle" });
    expect(h.past).toEqual([a]);
    h = historyReducer(h, { type: "undo" });
    expect(h.present).toBe(a);
  });

  it("forgets a drag that ended where it started", () => {
    const a = emptyDoc();
    let h = historyReducer(startHistory(a), { type: "preview", doc: step(a) });
    h = historyReducer(h, { type: "preview", doc: a });
    h = historyReducer(h, { type: "settle" });
    expect(h.past).toHaveLength(0);
  });

  it("merges a quick run of changes with the same key", () => {
    const a = emptyDoc();
    const b = step(a);
    const c = step(b);
    const d = step(c);
    let h = startHistory(a);
    h = historyReducer(h, { type: "commit", doc: b, key: "opacity:x", at: 0 });
    h = historyReducer(h, { type: "commit", doc: c, key: "opacity:x", at: 300 });
    expect(h.past).toEqual([a]);
    // Too late to merge, or another key: a new step.
    h = historyReducer(h, { type: "commit", doc: d, key: "opacity:x", at: 300 + MERGE_MS });
    expect(h.past).toEqual([a, c]);
    h = historyReducer(h, { type: "commit", doc: step(d), key: "size:x", at: 300 + MERGE_MS + 1 });
    expect(h.past).toHaveLength(3);
  });

  it("keeps at most LIMIT steps", () => {
    let d = emptyDoc();
    let h = startHistory(d);
    for (let i = 0; i < LIMIT + 20; i++) {
      d = { ...d };
      h = historyReducer(h, { type: "commit", doc: d });
    }
    expect(h.past).toHaveLength(LIMIT);
  });

  it("starts again from a new document", () => {
    const a = emptyDoc();
    const h = historyReducer(historyReducer(startHistory(a), { type: "commit", doc: step(a) }), { type: "reset", doc: a });
    expect(h.past).toHaveLength(0);
    expect(h.present).toBe(a);
  });
});
