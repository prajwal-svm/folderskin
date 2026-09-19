/**
 * The first-launch onboarding's decisions, kept apart from the screens so they can be tested:
 * which skin each folder of the intro wears, which packs start picked, and how installing them
 * reads.
 */
import type { PackProgress } from "./tauri";

/** The pack a first launch starts with, when GitHub lists it. */
export const DEFAULT_PACK = "classic-art";

/** How many folders the intro shows side by side; the middle one is the biggest. */
export const INTRO_SLOTS = 5;

/**
 * The frame folder `slot` wears at `step` of the intro, out of `frames`. The five folders never
 * wear the same frame at once, and each one changes at every step.
 */
export function frameAt(slot: number, step: number, frames: number): number {
  return (slot * 2 + step * 5) % frames;
}

/** Where one pack is in being installed. */
export type InstallState =
  | { kind: "queued" }
  | { kind: "download"; done: number; total: number }
  | { kind: "save"; done: number; total: number }
  | { kind: "done"; count: number }
  | { kind: "failed"; error: string };

export function fromProgress(p: PackProgress): InstallState {
  return { kind: p.stage, done: Math.max(0, Math.min(p.done, p.total)), total: Math.max(0, p.total) };
}

/** How far along a pack is, from 0 to 1. Downloading is most of the wait, so it fills 80%. */
export function installFraction(state: InstallState | undefined): number {
  if (!state) return 0;
  switch (state.kind) {
    case "queued":
    case "failed":
      return 0;
    case "download":
      return state.total ? 0.8 * (state.done / state.total) : 0;
    case "save":
      return 0.8 + (state.total ? 0.2 * (state.done / state.total) : 0);
    case "done":
      return 1;
  }
}

/** The line under a pack while it's being installed, or after. */
export function installLine(state: InstallState): string {
  switch (state.kind) {
    case "queued":
      return "Waiting…";
    case "download":
      return state.total ? `Downloading ${state.done} of ${state.total}` : "Downloading…";
    case "save":
      return "Adding to your library…";
    case "done":
      return `Added ${state.count} ${state.count === 1 ? "skin" : "skins"}`;
    case "failed":
      return `Couldn't add it: ${state.error}`;
  }
}

type PackLike = { id: string; name: string; added: boolean };

/** The packs picked when the list arrives: Classic Art, or else the first pack not added yet. */
export function defaultPicks(packs: PackLike[]): string[] {
  const open = packs.filter((p) => !p.added);
  const pick = open.find((p) => p.id === DEFAULT_PACK) ?? open[0];
  return pick ? [pick.id] : [];
}

/**
 * The picked packs the main button installs, in the order they're listed: not added yet, and not
 * already tried and failed (those have their own Try again).
 */
export function toInstall<P extends PackLike>(packs: P[], picked: ReadonlySet<string>, failed: ReadonlySet<string> = new Set()): P[] {
  return packs.filter((p) => picked.has(p.id) && !p.added && !failed.has(p.id));
}

/** The words on the main button of the packs step while nothing is installing. */
export function continueLabel(next: PackLike[], anyAdded: boolean): string {
  if (next.length === 1) return `Add ${next[0].name}`;
  if (next.length > 1) return `Add ${next.length} packs`;
  return anyAdded ? "Continue" : "Continue without packs";
}
