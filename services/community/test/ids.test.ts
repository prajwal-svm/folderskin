import { afterEach, describe, expect, it, vi } from "vitest";
import { isGeneratedId, isPackId, newPackId } from "../src/text";
import { drawSuffixes } from "./helpers";

afterEach(() => vi.restoreAllMocks());

const nothingTaken = () => false;

describe("a new pack id", () => {
  it("is the name's slug, a dash and six random characters", async () => {
    const id = await newPackId("Classic Art", nothingTaken);
    expect(id).toMatch(/^classic-art-[a-z2-7]{6}$/);
    expect(isPackId(id)).toBe(true);
    expect(isGeneratedId(id)).toBe(true);
  });

  it("takes each character from a random byte's low five bits", async () => {
    drawSuffixes([0, 1, 25, 26, 31, 32 + 7]);
    expect(await newPackId("Koi", nothingTaken)).toBe("koi-abz27h");
  });

  it("is as likely to end in any character as in any other: every byte maps to one of 32, eight bytes to each", async () => {
    // 128 ids of 6 random bytes go through every byte value three times.
    const bytes = Array.from({ length: 128 }, (_, i) => Array.from({ length: 6 }, (_, j) => (i * 6 + j) % 256));
    drawSuffixes(...bytes);
    const seen = new Map<string, number>();
    for (let i = 0; i < bytes.length; i++) {
      for (const c of (await newPackId("Koi", nothingTaken)).slice(4)) seen.set(c, (seen.get(c) ?? 0) + 1);
    }
    expect([...seen.keys()].sort().join("")).toBe("234567abcdefghijklmnopqrstuvwxyz");
    expect(new Set(seen.values())).toEqual(new Set([24]));
  });

  it("draws from a real random source when nothing stands in for it", async () => {
    // Two of 100 ids alike would be a one in 200,000 chance.
    const ids = new Set<string>();
    for (let i = 0; i < 100; i++) ids.add(await newPackId("Koi", nothingTaken));
    expect(ids.size).toBe(100);
    for (const id of ids) expect(id).toMatch(/^koi-[a-z2-7]{6}$/);
  });

  it("cuts a long name's slug to 33 characters without leaving a dash at the end, so the id fits in 40", async () => {
    const long = await newPackId("The quick brown fox jumps over the lazy dog again", nothingTaken);
    expect(long).toMatch(/^the-quick-brown-fox-jumps-over-th-[a-z2-7]{6}$/);
    expect(long).toHaveLength(40);
    // The 33rd character is a dash, which goes.
    expect(await newPackId("abcdefghij abcdefghij abcdefghij x", nothingTaken)).toMatch(/^abcdefghij-abcdefghij-abcdefghij-[a-z2-7]{6}$/);
  });

  it("starts with pack when the name has no letters or digits, and keeps clear of Windows' device names", async () => {
    expect(await newPackId("!!! ???", nothingTaken)).toMatch(/^pack-[a-z2-7]{6}$/);
    expect(await newPackId("CON", nothingTaken)).toMatch(/^con-1-[a-z2-7]{6}$/);
  });

  it("draws again while the id it drew is taken, and gives up rather than try for ever", async () => {
    drawSuffixes([0, 0, 0, 0, 0, 0], [1, 1, 1, 1, 1, 1], [2, 2, 2, 2, 2, 2]);
    const asked: string[] = [];
    const id = await newPackId("Koi", async (candidate) => {
      asked.push(candidate);
      return asked.length < 3;
    });
    expect(asked).toEqual(["koi-aaaaaa", "koi-bbbbbb", "koi-cccccc"]);
    expect(id).toBe("koi-cccccc");
    await expect(newPackId("Koi", () => true)).rejects.toThrow(/no free pack id/);
  });
});

describe("the shape of a generated id", () => {
  it("is what newPackId makes, as folderskin_core's is_generated_id reads it", () => {
    for (const id of ["classic-art-k7q2mx", "pack-aaaaaa", "a-222222", "con-1-abcdef", `${"a".repeat(33)}-abcdef`]) {
      expect(isGeneratedId(id), id).toBe(true);
    }
  });

  it("is nothing else", () => {
    for (const id of [
      "classic-art",
      "classic-art-k7q2m",
      "classic-art-k7q2mxx",
      "classic-art-k7q2m1",
      "classic-art-K7Q2MX",
      "k7q2mx",
      "-k7q2mx",
      "classic--art-k7q2mx",
      `${"a".repeat(34)}-abcdef`,
      42,
    ]) {
      expect(isGeneratedId(id), String(id)).toBe(false);
    }
  });

  it("is only a shape: an id from before can have it by chance", () => {
    expect(isGeneratedId("night-prints-bright")).toBe(true);
  });
});
