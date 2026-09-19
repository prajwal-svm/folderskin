import { describe, expect, it } from "vitest";
import { makeShape, SHAPES } from "./doc";
import { heartPoints, polygonPoints, starPoints, traceShape, type PathSink } from "./shapes";

/** Records the points a shape traces, arcs and ellipses as their end points. */
function recorder() {
  const points: [number, number][] = [];
  let closed = 0;
  const sink: PathSink = {
    moveTo: (x, y) => void points.push([x, y]),
    lineTo: (x, y) => void points.push([x, y]),
    arcTo: (_x1, _y1, x2, y2) => void points.push([x2, y2]),
    ellipse: (x, y, rx) => void points.push([x + rx, y]),
    closePath: () => void (closed += 1),
  };
  return { sink, points, closed: () => closed };
}

describe("shapes", () => {
  it("puts a star's tips on the outside and its valleys inside", () => {
    const pts = starPoints(200, 200, 5, 0.5);
    expect(pts).toHaveLength(10);
    expect(pts[0][0]).toBeCloseTo(0, 6);
    expect(pts[0][1]).toBeCloseTo(-100, 6);
    expect(Math.hypot(...pts[1])).toBeCloseTo(50, 6);
  });

  it("makes regular polygons with a corner at the top", () => {
    const hex = polygonPoints(100, 100, 6);
    expect(hex).toHaveLength(6);
    for (const p of hex) expect(Math.hypot(...p)).toBeCloseTo(50, 6);
    expect(polygonPoints(10, 10, 1)).toHaveLength(3);
  });

  it("fits the heart to its box", () => {
    const pts = heartPoints(300, 200);
    const xs = pts.map((p) => p[0]);
    const ys = pts.map((p) => p[1]);
    expect(Math.max(...xs) - Math.min(...xs)).toBeCloseTo(300, 0);
    expect(Math.max(...ys) - Math.min(...ys)).toBeCloseTo(200, 0);
  });

  it("traces every shape inside its box, closed", () => {
    for (const { id } of SHAPES) {
      const layer = { ...makeShape(id, 0, 0, "#000"), w: 300, h: 200 };
      const r = recorder();
      traceShape(r.sink, layer);
      expect(r.closed(), id).toBeGreaterThan(0);
      for (const [x, y] of r.points) {
        expect(Math.abs(x), id).toBeLessThanOrEqual(150.001);
        expect(Math.abs(y), id).toBeLessThanOrEqual(100.001);
      }
    }
  });
});
