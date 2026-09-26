/**
 * The latest run over a folder's tree, as the app tells of it (src-tauri/src/tree.rs). A run goes
 * on in the background whatever the window shows, so it lives here rather than in a component:
 * the folder panel, the sidebar's run indicator and the composer all read it from here.
 *
 * The app numbers what it says about runs, and a message that arrives after a newer one is
 * dropped. A run is heard to end only when it was heard going first, so a run that ended before
 * the window opened isn't announced again.
 */
import { useSyncExternalStore } from "react";
import type { TreeRun, TreeRunEvent } from "../lib/tree";

export type RunStore = ReturnType<typeof createRunStore>;

export function createRunStore() {
  let seq = -1;
  let run: TreeRun | null = null;
  /** Runs heard going and not yet heard ending, by id. */
  const going = new Set<number>();
  /** How many folders runs take by what the count said as they started, by id. */
  const expecting = new Map<number, number>();
  const listeners = new Set<() => void>();
  const endListeners = new Set<(run: TreeRun) => void>();

  const onEnded = (listener: (run: TreeRun) => void) => {
    endListeners.add(listener);
    return () => void endListeners.delete(listener);
  };

  return {
    /** The latest run, or null. */
    now: () => run,

    /** What the app said. Older than what's known already, it changes nothing. */
    take(event: TreeRunEvent) {
      if (event.seq <= seq) return;
      seq = event.seq;
      run = event.run;
      const ended = run !== null && !run.running && going.delete(run.id) ? run : null;
      if (run?.running) going.add(run.id);
      for (const listener of [...listeners]) listener();
      if (ended) for (const listener of [...endListeners]) listener(ended);
    },

    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },

    /**
     * Run `id` takes `total` folders, as a count that had finished before it started says. The
     * run's own walk finds them again, and until it has, its count reads this rather than "+".
     */
    expect(id: number, total: number) {
      expecting.clear();
      expecting.set(id, total);
      // Said about the run already on show, whoever shows it draws it again with this.
      if (run?.id !== id) return;
      run = { ...run };
      for (const listener of [...listeners]) listener();
    },

    /** How many folders run `id` takes by the count that had finished as it started, if one had. */
    expected: (id: number): number | null => expecting.get(id) ?? null,

    /** Hears each run that was going end. Returns what stops hearing it. */
    onEnded,

    /** Resolves to run `id` once it has ended: at once if it has already. */
    whenEnded(id: number): Promise<TreeRun> {
      if (run?.id === id && !run.running) return Promise.resolve(run);
      return new Promise((resolve) => {
        const stop = onEnded((ended) => {
          if (ended.id !== id) return;
          stop();
          resolve(ended);
        });
      });
    },
  };
}

/** The app's latest run. */
export const treeRuns = createRunStore();

/** The latest run, for a component, which draws again whenever it changes. */
export function useTreeRun(): TreeRun | null {
  return useSyncExternalStore(treeRuns.subscribe, treeRuns.now, treeRuns.now);
}
