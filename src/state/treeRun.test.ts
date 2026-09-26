import { describe, expect, it } from "vitest";
import { createRunStore } from "./treeRun";
import type { TreeRun } from "../lib/tree";

const run = (patch: Partial<TreeRun>): TreeRun => ({
  id: 1,
  kind: "apply",
  undoing: false,
  leaves_plain: false,
  folder: "/Users/me/Projects",
  root: "/Users/me/Projects",
  name: "Projects",
  skin_id: "user:coral",
  running: true,
  stopping: false,
  stopped: false,
  done: 0,
  total: 1,
  counted: false,
  current: "Projects",
  changed: 0,
  failed: 0,
  skipped: 0,
  failures: [],
  error: null,
  ...patch,
});

describe("the latest run, as the app tells of it", () => {
  it("keeps the newest message, whatever order they arrive in", () => {
    const store = createRunStore();
    let heard = 0;
    store.subscribe(() => (heard += 1));
    store.take({ seq: 5, run: run({ done: 40 }) });
    store.take({ seq: 3, run: run({ done: 10 }) });
    expect(store.now()?.done).toBe(40);
    expect(heard).toBe(1);
    store.take({ seq: 6, run: null });
    expect(store.now()).toBeNull();
  });

  it("hears a run end only when it was heard going", () => {
    const store = createRunStore();
    const ended: number[] = [];
    store.onEnded((r) => ended.push(r.id));
    // Ended before the window heard of it: nothing to announce.
    store.take({ seq: 1, run: run({ id: 1, running: false, done: 5, changed: 5 }) });
    expect(ended).toEqual([]);
    store.take({ seq: 2, run: run({ id: 2 }) });
    store.take({ seq: 3, run: run({ id: 2, done: 3 }) });
    store.take({ seq: 4, run: run({ id: 2, running: false, done: 5, changed: 5 }) });
    // Said again, it's not a second ending.
    store.take({ seq: 5, run: run({ id: 2, running: false, done: 5, changed: 5 }) });
    expect(ended).toEqual([2]);
    // Carried on, it goes and ends again.
    store.take({ seq: 6, run: run({ id: 2, done: 5 }) });
    store.take({ seq: 7, run: run({ id: 2, running: false, done: 9, changed: 9 }) });
    expect(ended).toEqual([2, 2]);
  });

  it("keeps how many folders the latest run was expected to take", () => {
    const store = createRunStore();
    expect(store.expected(4)).toBeNull();
    store.expect(4, 29);
    expect(store.expected(4)).toBe(29);
    // A newer run's expectation is the only one kept.
    store.expect(5, 3);
    expect([store.expected(4), store.expected(5)]).toEqual([null, 3]);
  });

  it("draws the run on show again when it's said how many folders it takes", () => {
    const store = createRunStore();
    store.take({ seq: 1, run: run({ id: 7 }) });
    const before = store.now();
    let heard = 0;
    store.subscribe(() => (heard += 1));
    store.expect(7, 48_211);
    expect(heard).toBe(1);
    expect(store.now()).not.toBe(before);
    expect(store.now()?.id).toBe(7);
    // About a run not on show yet, there's nothing to draw.
    store.expect(8, 12);
    expect(heard).toBe(1);
  });

  it("resolves when a run ends, or at once when it has", async () => {
    const store = createRunStore();
    store.take({ seq: 1, run: run({ id: 4 }) });
    const later = store.whenEnded(4);
    store.take({ seq: 2, run: run({ id: 4, running: false, done: 1, changed: 1 }) });
    expect((await later).changed).toBe(1);
    expect((await store.whenEnded(4)).id).toBe(4);
  });
});
