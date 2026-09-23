import { describe, expect, it } from "vitest";
import { CENTRE_MIN, columns, DEFAULT_LAYOUT, defaultRight, dragRight, dragSidebar, FOLD_AT, GAP, LEFT, RAIL, readLayout, RIGHT } from "./layout";

describe("readLayout", () => {
  it("falls back to the defaults for nothing, junk or the wrong types", () => {
    expect(readLayout(null)).toEqual(DEFAULT_LAYOUT);
    expect(readLayout("{not json")).toEqual(DEFAULT_LAYOUT);
    expect(readLayout(JSON.stringify({ left: "wide", rail: "yes", right: {} }))).toEqual(DEFAULT_LAYOUT);
  });

  it("keeps saved widths within their limits", () => {
    expect(readLayout(JSON.stringify({ left: 5000, rail: true, right: 10 }))).toEqual({ left: LEFT.max, rail: true, right: RIGHT.min });
  });
});

describe("columns", () => {
  it("gives the right island its default width in a roomy window", () => {
    expect(columns(DEFAULT_LAYOUT, 1440, true)).toEqual({ left: LEFT.initial, right: defaultRight(1440) });
    expect(defaultRight(1440)).toBe(372);
    expect(defaultRight(1040)).toBe(320);
  });

  it("takes the right island from a narrow window before the centre", () => {
    const layout = { ...DEFAULT_LAYOUT, left: LEFT.max, right: RIGHT.max };
    const w = 1040;
    const { left, right } = columns(layout, w, true);
    expect(w - left - right - 3 * GAP).toBeGreaterThanOrEqual(CENTRE_MIN);
    expect(right).toBeGreaterThanOrEqual(RIGHT.min);
  });

  it("gives the rail its own width and no right island when it's hidden", () => {
    expect(columns({ ...DEFAULT_LAYOUT, rail: true }, 1200, false)).toEqual({ left: RAIL, right: 0 });
  });
});

describe("dragging the sidebar", () => {
  it("resizes within its limits", () => {
    expect(dragSidebar(DEFAULT_LAYOUT, 260).left).toBe(260);
    expect(dragSidebar(DEFAULT_LAYOUT, 180)).toEqual({ ...DEFAULT_LAYOUT, left: LEFT.min });
    expect(dragSidebar(DEFAULT_LAYOUT, 900).left).toBe(LEFT.max);
  });

  it("folds to the rail when dragged narrow, keeping the width it had to open back to", () => {
    const folded = dragSidebar({ ...DEFAULT_LAYOUT, left: 250 }, FOLD_AT - 1);
    expect(folded).toEqual({ left: 250, rail: true, right: null });
  });

  it("opens a rail dragged wide again", () => {
    const rail = { ...DEFAULT_LAYOUT, rail: true };
    expect(dragSidebar(rail, 100)).toBe(rail);
    expect(dragSidebar(rail, 240)).toEqual({ ...DEFAULT_LAYOUT, left: 240 });
  });
});

describe("dragging the right island", () => {
  it("stays within its limits and leaves the centre its room", () => {
    expect(dragRight(DEFAULT_LAYOUT, 100, 1440).right).toBe(RIGHT.min);
    expect(dragRight(DEFAULT_LAYOUT, 2000, 1440).right).toBe(RIGHT.max);
    const narrow = dragRight(DEFAULT_LAYOUT, 470, 1100).right!;
    expect(1100 - LEFT.initial - narrow - 3 * GAP).toBeGreaterThanOrEqual(CENTRE_MIN);
  });
});
