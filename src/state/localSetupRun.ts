import { useSyncExternalStore } from "react";
import type { AiEvent } from "./chats";

/**
 * Whether the Local Model is being set up, and how far it has got. A setup outlives the panel
 * that started it (it goes on while another provider is shown, or the dialog is closed and opened
 * again), so the provider list asks here rather than the panel, to show that it's downloading,
 * and the panel opened again part-way carries on counting from here rather than from nothing.
 */
let runs = 0;
const listeners = new Set<() => void>();

/**
 * A setup under way, as this window has heard it: the stage, the file coming in, and for the
 * whole download how much there was to fetch when it started and how much of each file has come
 * since (from where that file resumed).
 */
export type SetupProgress = {
  stage: string;
  file: string | null;
  done: number;
  total: number;
  whole: number;
  from: Record<string, number>;
  got: Record<string, number>;
};

let progress: SetupProgress | null = null;

function tell() {
  for (const listener of listeners) listener();
}

/**
 * A setup started, or was joined, with `whole` bytes to download as far as this window knows;
 * the function returned says it ended, however it ended (once). Joining one this window is
 * already counting keeps its count.
 */
export function setupBegan(whole = 0): () => void {
  runs += 1;
  progress ??= { stage: "Getting ready", file: null, done: 0, total: 0, whole, from: {}, got: {} };
  tell();
  let ended = false;
  return () => {
    if (ended) return;
    ended = true;
    runs -= 1;
    if (runs === 0) progress = null;
    tell();
  };
}

/** What `event` changes of `setup`. A new stage is about something else, so the file line goes. */
export function heardIn(setup: SetupProgress, event: AiEvent): SetupProgress {
  switch (event.type) {
    case "stage":
      return { ...setup, stage: event.message, file: null, done: 0, total: 0 };
    case "download": {
      // A file that resumes starts where it left off: only what comes now counts.
      const from = event.file in setup.from ? setup.from : { ...setup.from, [event.file]: event.done };
      const got = { ...setup.got, [event.file]: Math.max(0, event.done - from[event.file]) };
      return { ...setup, file: event.file, done: event.done, total: event.total, from, got };
    }
    case "progress":
      return { ...setup, done: event.step, total: event.steps };
    case "log":
      return setup;
  }
}

/** One of the setup's events, heard by any window's panel that asked for it. */
export function heard(event: AiEvent) {
  if (!progress) return;
  const next = heardIn(progress, event);
  if (next === progress) return;
  progress = next;
  tell();
}

/** How much of the whole download has come so far. */
export const wholeDone = (setup: SetupProgress) => Object.values(setup.got).reduce((sum, n) => sum + n, 0);

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Whether a setup is running, re-rendering as one starts and ends. */
export function useLocalSetupRunning(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => runs > 0,
    () => false,
  );
}

/** How far the setup under way has got, as this window has heard it; null when none is. */
export const setupProgress = (): SetupProgress | null => progress;

/** [`setupProgress`], re-rendering as it changes. */
export function useSetupProgress(): SetupProgress | null {
  return useSyncExternalStore(subscribe, setupProgress, () => null);
}
