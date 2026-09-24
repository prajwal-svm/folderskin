import { useSyncExternalStore } from "react";

/**
 * Whether the Local Model is being set up. A setup outlives the panel that started it (it goes
 * on while another provider is shown, or the dialog is closed and opened again), so the provider
 * list asks here rather than the panel, to show that it's downloading.
 */
let runs = 0;
const listeners = new Set<() => void>();

function tell() {
  for (const listener of listeners) listener();
}

/** A setup started, or was joined; the function returned says it ended, however it ended (once). */
export function setupBegan(): () => void {
  runs += 1;
  tell();
  let ended = false;
  return () => {
    if (ended) return;
    ended = true;
    runs -= 1;
    tell();
  };
}

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
