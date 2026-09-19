import { describe, expect, it } from "vitest";
import {
  continueLabel,
  defaultPicks,
  frameAt,
  fromProgress,
  INTRO_FRAME_COUNT,
  INTRO_SLOTS,
  INTRO_SPINS,
  INTRO_WAVES,
  installFraction,
  installLine,
  spinFrameAt,
  toInstall,
} from "./onboarding";
import { INTRO } from "../assets/onboarding";

const classic = { id: "classic-art", name: "Classic Art", added: false };
const colours = { id: "colours", name: "Colours", added: false };
const night = { id: "night-prints", name: "Night prints", added: false };

/** Every frame the intro shows, in order: each wave's folders left to right, then the flips. */
function introOrder(): number[] {
  const waves = Array.from({ length: INTRO_WAVES }, (_, wave) => Array.from({ length: INTRO_SLOTS }, (_, slot) => frameAt(slot, wave)));
  const spins = Array.from({ length: INTRO_SPINS }, (_, i) => spinFrameAt(i + 1));
  return [...waves.flat(), ...spins];
}

describe("frameAt and spinFrameAt", () => {
  it("show every frame exactly once", () => {
    const order = introOrder();
    expect(order).toHaveLength(INTRO_FRAME_COUNT);
    expect([...order].sort((a, b) => a - b)).toEqual(Array.from({ length: INTRO_FRAME_COUNT }, (_, i) => i));
  });
});

describe("the intro's frames", () => {
  const pictures = ["classic-art", "scientists-pop-art", "statues"];
  const middle = Math.floor(INTRO_SLOTS / 2);

  it("are one picture for every folder of every wave and every flip", () => {
    expect(INTRO).toHaveLength(INTRO_FRAME_COUNT);
    expect(new Set(INTRO.map((f) => f.src)).size).toBe(INTRO.length);
  });

  it("never put two folders from one pack side by side, and keep plain folders to the ends", () => {
    for (let w = 0; w < INTRO_WAVES; w++) {
      const shown = Array.from({ length: INTRO_SLOTS }, (_, slot) => INTRO[frameAt(slot, w)].pack);
      const wave = `wave ${w}: ${shown.join(", ")}`;
      shown.slice(1).forEach((pack, i) => expect(pack, `${wave}, slots ${i} and ${i + 1}`).not.toBe(shown[i]));
      expect(pictures, wave).toContain(shown[middle]);
      const plain = shown.flatMap((pack, slot) => (pictures.includes(pack) ? [] : [slot]));
      expect(plain.length, wave).toBeLessThanOrEqual(1);
      for (const slot of plain) expect([0, INTRO_SLOTS - 1], wave).toContain(slot);
    }
  });

  it("flip the middle folder through pictures, never two from one pack in a row", () => {
    const flips = [frameAt(middle, INTRO_WAVES - 1), ...Array.from({ length: INTRO_SPINS }, (_, i) => spinFrameAt(i + 1))].map((f) => INTRO[f].pack);
    for (const pack of flips) expect(pictures).toContain(pack);
    flips.slice(1).forEach((pack, i) => expect(pack, `flips ${i} and ${i + 1}`).not.toBe(flips[i]));
  });

  it("come from every community pack", () => {
    expect(new Set(INTRO.map((f) => f.pack))).toEqual(new Set(["classic-art", "scientists-pop-art", "statues", "soft-rainbow", "colours"]));
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
