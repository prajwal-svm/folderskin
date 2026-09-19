import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { throttle } from "./throttle";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("progress at a pace people can read", () => {
  it("passes the first value on at once, then the latest of each wait", () => {
    const sent: number[] = [];
    const t = throttle<number>((v) => sent.push(v), 50);
    t.push(1);
    expect(sent).toEqual([1]);
    t.push(2);
    t.push(3);
    expect(sent).toEqual([1]);
    vi.advanceTimersByTime(50);
    expect(sent).toEqual([1, 3]);
    vi.advanceTimersByTime(200);
    t.push(4);
    expect(sent).toEqual([1, 3, 4]);
  });

  it("sends at most once a wait however fast values come", () => {
    const sent: number[] = [];
    const t = throttle<number>((v) => sent.push(v), 50);
    for (let i = 1; i <= 1000; i++) {
      t.push(i);
      vi.advanceTimersByTime(1);
    }
    vi.advanceTimersByTime(50);
    expect(sent.length).toBeLessThanOrEqual(21);
    expect(sent.at(-1)).toBe(1000);
  });

  it("drops what's waiting once the run is over", () => {
    const sent: number[] = [];
    const t = throttle<number>((v) => sent.push(v), 50);
    t.push(1);
    t.push(2);
    t.cancel();
    vi.advanceTimersByTime(100);
    expect(sent).toEqual([1]);
  });
});
