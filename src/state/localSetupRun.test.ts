import { describe, expect, it } from "vitest";
import { heard, heardIn, setupBegan, setupProgress, wholeDone, type SetupProgress } from "./localSetupRun";

const MB = 1_000_000;
const fresh = (stage: string): SetupProgress => ({ stage, file: null, done: 0, total: 0, whole: 500 * MB, from: {}, got: {} });

describe("a Local Model setup's progress", () => {
  it("counts each file from where it resumed", () => {
    let p = fresh("Getting ready");
    p = heardIn(p, { type: "download", file: "a.gguf", done: 100 * MB, total: 300 * MB });
    p = heardIn(p, { type: "download", file: "a.gguf", done: 250 * MB, total: 300 * MB });
    expect(wholeDone(p)).toBe(150 * MB);
    // The same event heard twice (a panel that joined hears it too) counts once.
    p = heardIn(p, { type: "download", file: "a.gguf", done: 250 * MB, total: 300 * MB });
    expect(wholeDone(p)).toBe(150 * MB);
  });

  it("names no file under a stage about another one", () => {
    let p = fresh("Downloading a.gguf");
    p = heardIn(p, { type: "download", file: "a.gguf", done: 0, total: 300 * MB });
    p = heardIn(p, { type: "download", file: "a.gguf", done: 300 * MB, total: 300 * MB });
    p = heardIn(p, { type: "stage", stage: "download", message: "Downloading b.gguf" });
    expect([p.stage, p.file, p.done, p.total]).toEqual(["Downloading b.gguf", null, 0, 0]);
    // What came of the file before still counts in the whole.
    expect(wholeDone(p)).toBe(300 * MB);
  });

  it("counts on when the panel joins the setup again, and forgets it once every panel has heard it end", () => {
    const first = setupBegan(500 * MB);
    heard({ type: "download", file: "a.gguf", done: 0, total: 300 * MB });
    heard({ type: "download", file: "a.gguf", done: 120 * MB, total: 300 * MB });
    // Closed and opened again, the panel joins: less is left to fetch now, but the count goes on
    // from what this window has already heard, and the first panel's channel still hears it too.
    const again = setupBegan(380 * MB);
    heard({ type: "download", file: "a.gguf", done: 150 * MB, total: 300 * MB });
    heard({ type: "download", file: "a.gguf", done: 150 * MB, total: 300 * MB });
    const p = setupProgress()!;
    expect([p.whole, wholeDone(p)]).toEqual([500 * MB, 150 * MB]);
    first();
    expect(setupProgress()).not.toBeNull();
    again();
    expect(setupProgress()).toBeNull();
    // Nothing is heard with no setup, and a later one starts from nothing.
    heard({ type: "download", file: "a.gguf", done: 300 * MB, total: 300 * MB });
    expect(setupProgress()).toBeNull();
    const later = setupBegan(200 * MB);
    expect([setupProgress()?.whole, wholeDone(setupProgress()!)]).toEqual([200 * MB, 0]);
    later();
  });
});
