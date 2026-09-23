import { describe, expect, it } from "vitest";
import { cellPosition, gridLayout, scrollToCell, visibleRange } from "./virtual";

const square = (w: number) => w;

describe("gridLayout", () => {
  it("fits as many columns as the minimum cell width allows", () => {
    const l = gridLayout(330, 100, 50, 10, square);
    expect(l.columns).toBe(5); // 5*50 + 4*10 = 290 ≤ 330; 6 would need 350
    expect(l.cellWidth).toBeCloseTo((330 - 40) / 5);
    expect(l.rows).toBe(20);
    expect(l.height).toBeCloseTo(20 * l.cellWidth + 19 * 10);
  });

  it("keeps one column when even one cell doesn't fit", () => {
    const l = gridLayout(30, 3, 50, 10, () => 20);
    expect(l.columns).toBe(1);
    expect(l.cellWidth).toBe(30);
    expect(l.height).toBe(3 * 20 + 2 * 10);
  });

  it("is empty with nothing to show", () => {
    const l = gridLayout(300, 0, 50, 10, square);
    expect(l.rows).toBe(0);
    expect(l.height).toBe(0);
  });
});

describe("visibleRange", () => {
  const l = gridLayout(330, 1000, 50, 10, () => 40); // 5 columns, rows 50 apart

  it("draws the rows on screen plus the overscan", () => {
    expect(visibleRange(l, 1000, 0, 200, 2)).toEqual({ start: 0, end: 5 * 7 }); // rows 0-4 visible, +2
    const mid = visibleRange(l, 1000, 1000, 200, 1); // rows 20-24 visible
    expect(mid).toEqual({ start: 19 * 5, end: 26 * 5 });
  });

  it("stops at the last item", () => {
    const r = visibleRange(l, 1000, 1e9, 200, 2);
    expect(r.end).toBe(1000);
    expect(r.start).toBeLessThan(1000);
  });

  it("draws nothing for an empty grid", () => {
    expect(visibleRange(gridLayout(300, 0, 50, 10, square), 0, 0, 500)).toEqual({ start: 0, end: 0 });
  });
});

describe("positions and scrolling to a cell", () => {
  const l = gridLayout(330, 100, 50, 10, () => 40);

  it("places cells in reading order", () => {
    expect(cellPosition(l, 0)).toEqual({ x: 0, y: 0 });
    expect(cellPosition(l, 6)).toEqual({ x: l.cellWidth + 10, y: 50 });
  });

  it("scrolls only as far as it takes to show the cell", () => {
    expect(scrollToCell(l, 0, 0, 200)).toBe(0); // already visible
    expect(scrollToCell(l, 5 * 10, 0, 200)).toBe(10 * 50 + 40 - 200); // below: align bottom
    expect(scrollToCell(l, 0, 300, 200)).toBe(0); // above: align top
  });
});
