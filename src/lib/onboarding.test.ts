import { describe, expect, it } from "vitest";
import {
  continueLabel,
  defaultPicks,
  frameAt,
  fromProgress,
  INTRO_SLOTS,
  installFraction,
  installLine,
  toInstall,
} from "./onboarding";

const classic = { id: "classic-art", name: "Classic Art", added: false };
const colours = { id: "colours", name: "Colours", added: false };
const night = { id: "night-prints", name: "Night prints", added: false };

describe("frameAt", () => {
  it("never shows one frame on two folders at once", () => {
    for (let step = 0; step < 12; step++) {
      const frames = Array.from({ length: INTRO_SLOTS }, (_, slot) => frameAt(slot, step, 12));
      expect(new Set(frames).size).toBe(INTRO_SLOTS);
    }
  });

  it("changes every folder at every step", () => {
    for (let slot = 0; slot < INTRO_SLOTS; slot++) {
      for (let step = 0; step < 12; step++) expect(frameAt(slot, step + 1, 12)).not.toBe(frameAt(slot, step, 12));
    }
  });
});

describe("defaultPicks", () => {
  it("picks Classic Art wherever it's listed", () => {
    expect(defaultPicks([colours, classic])).toEqual(["classic-art"]);
  });

  it("falls back to the first pack not added yet, or nothing", () => {
    expect(defaultPicks([colours, night])).toEqual(["colours"]);
    expect(defaultPicks([{ ...classic, added: true }, colours])).toEqual(["colours"]);
    expect(defaultPicks([{ ...classic, added: true }])).toEqual([]);
    expect(defaultPicks([])).toEqual([]);
  });
});

describe("toInstall and continueLabel", () => {
  it("names one pack and counts several", () => {
    const packs = [classic, colours];
    expect(continueLabel(toInstall(packs, new Set(["classic-art"])), false)).toBe("Add Classic Art");
    expect(continueLabel(toInstall(packs, new Set(["classic-art", "colours"])), false)).toBe("Add 2 packs");
  });

  it("keeps the list's order, not the order they were picked in", () => {
    expect(toInstall([classic, colours, night], new Set(["night-prints", "classic-art"]))).toEqual([classic, night]);
  });

  it("leaves out packs already added and packs that failed", () => {
    const packs = [{ ...classic, added: true }, colours, night];
    const picked = new Set(["classic-art", "colours", "night-prints"]);
    expect(toInstall(packs, picked)).toEqual([colours, night]);
    expect(toInstall(packs, picked, new Set(["colours"]))).toEqual([night]);
    expect(continueLabel(toInstall(packs, new Set(["classic-art"])), true)).toBe("Continue");
  });

  it("says so when nothing will be added", () => {
    expect(continueLabel(toInstall([classic, colours], new Set()), false)).toBe("Continue without packs");
    expect(continueLabel([], false)).toBe("Continue without packs");
  });
});

describe("install progress", () => {
  it("fills 80% while downloading and the rest while saving", () => {
    expect(installFraction(undefined)).toBe(0);
    expect(installFraction({ kind: "queued" })).toBe(0);
    expect(installFraction(fromProgress({ stage: "download", done: 8, total: 16 }))).toBeCloseTo(0.4);
    expect(installFraction(fromProgress({ stage: "save", done: 4, total: 16 }))).toBeCloseTo(0.85);
    expect(installFraction({ kind: "done", count: 16 })).toBe(1);
    expect(installFraction({ kind: "failed", error: "offline" })).toBe(0);
  });

  it("keeps a report in range", () => {
    expect(fromProgress({ stage: "download", done: 20, total: 16 })).toEqual({ kind: "download", done: 16, total: 16 });
    expect(installFraction(fromProgress({ stage: "download", done: 0, total: 0 }))).toBe(0);
  });

  it("reads as a sentence", () => {
    expect(installLine({ kind: "queued" })).toBe("Waiting…");
    expect(installLine({ kind: "download", done: 3, total: 16 })).toBe("Downloading 3 of 16");
    expect(installLine({ kind: "save", done: 1, total: 16 })).toBe("Adding to your library…");
    expect(installLine({ kind: "done", count: 1 })).toBe("Added 1 skin");
    expect(installLine({ kind: "done", count: 16 })).toBe("Added 16 skins");
    expect(installLine({ kind: "failed", error: "couldn't reach GitHub" })).toBe("Couldn't add it: couldn't reach GitHub");
  });
});
