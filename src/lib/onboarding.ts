/**
 * The first-launch onboarding's decisions, kept apart from the screens so they can be tested:
 * which skin each folder of the intro wears, which packs start picked, and how installing them
 * reads.
 */
import type { PackProgress } from "./tauri";
import { clip } from "./names";

/** The pack a first launch starts with, when the list has it: Classic Art, by the id it was first
 *  published under. Packs can move to a new id, and the list says where each old one went. */
export const DEFAULT_PACK = "classic-art";

/** How many folders the intro shows side by side; the middle one is the biggest. */
export const INTRO_SLOTS = 5;
/** When all of them change to new skins, in ms from when the intro's pictures are ready. */
export const INTRO_WAVE_TIMES = [0, 950, 1850, 2750];
/** How many times all of them change to new skins. */
export const INTRO_WAVES = INTRO_WAVE_TIMES.length;
/** How many more skins the middle folder flips through on its way to the logo. */
export const INTRO_SPINS = 6;
/** Every folder in every wave and every flip wears a picture of its own: none shows twice. */
export const INTRO_FRAME_COUNT = INTRO_SLOTS * INTRO_WAVES + INTRO_SPINS;

/** The frame folder `slot` wears in wave `wave`: each wave takes the next five. */
export function frameAt(slot: number, wave: number): number {
  return wave * INTRO_SLOTS + slot;
}

/** The frame of the middle folder's `spin`th flip on its way to the logo, counting from 1. */
export function spinFrameAt(spin: number): number {
  return INTRO_SLOTS * INTRO_WAVES + spin - 1;
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
      return "Waiting";
    case "download":
      return state.total ? `Downloading ${state.done} of ${state.total}` : "Downloading";
    case "save":
      return "Adding to your library";
    case "done":
      return `Added ${state.count} ${state.count === 1 ? "skin" : "skins"}`;
    case "failed":
      return `Couldn't add it: ${state.error}`;
  }
}

type PackLike = { id: string; name: string; added: boolean };

/**
 * The pack picked when the list arrives: Classic Art under whatever id it has now (`moved` maps
 * each old id to the new one), or else the first pack not added yet, which is the first of the
 * featured ones when the list is those.
 */
export function defaultPick<P extends PackLike>(packs: P[], moved: Record<string, string> = {}): P | undefined {
  const open = packs.filter((p) => !p.added);
  const id = moved[DEFAULT_PACK] ?? DEFAULT_PACK;
  return open.find((p) => p.id === id) ?? open[0];
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
  if (next.length === 1) return `Add ${clip(next[0].name, 24)}`;
  if (next.length > 1) return `Add ${next.length} packs`;
  return anyAdded ? "Continue" : "Continue without packs";
}
