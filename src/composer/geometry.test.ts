import { describe, expect, it } from "vitest";
import { bounds, boxTargets, contains, cursorFor, handlePoint, hitHandle, MIN_SIZE, normAngle, resizeBox, rotateBy, snap, toLocal, toWorld, type Box } from "./geometry";

const box: Box = { x: 500, y: 400, w: 200, h: 100, rotation: 0 };
const turned: Box = { ...box, rotation: 90 };
const close = (a: number, b: number) => expect(a).toBeCloseTo(b, 6);

describe("canvas geometry", () => {
  it("goes between the canvas and a box's own frame", () => {
    const p = { x: 530, y: 420 };
    const l = toLocal(p, turned);
    close(l.x, 20);
    close(l.y, -30);
    const back = toWorld(l, turned);
    close(back.x, p.x);
    close(back.y, p.y);
  });

  it("finds whether a point is inside a turned box", () => {
    expect(contains(box, { x: 595, y: 445 })).toBe(true);
    expect(contains(box, { x: 500, y: 460 })).toBe(false);
    // Turned a quarter, the tall way is now across.
    expect(contains(turned, { x: 500, y: 490 })).toBe(true);
    expect(contains(turned, { x: 590, y: 400 })).toBe(false);
    expect(contains(box, { x: 605, y: 400 }, 6)).toBe(true);
  });

  it("measures the space a turned box covers", () => {
    expect(bounds(box)).toEqual({ x0: 400, y0: 350, x1: 600, y1: 450 });
    const b = bounds(turned);
    close(b.x0, 450);
    close(b.y0, 300);
  });

  it("places and finds handles", () => {
    expect(handlePoint(box, "se")).toEqual({ x: 600, y: 450 });
    const rot = handlePoint(box, "rot", 30);
    close(rot.x, 500);
    close(rot.y, 320);
    expect(hitHandle(box, { x: 603, y: 452 }, ["nw", "se"], 8, 30)).toBe("se");
    expect(hitHandle(box, { x: 500, y: 322 }, ["se", "rot"], 8, 30)).toBe("rot");
    expect(hitHandle(box, { x: 550, y: 400 }, ["nw", "se", "rot"], 8, 30)).toBeNull();
  });

  it("points resize cursors the way a turned box faces", () => {
    expect(cursorFor("e", 0)).toBe("ew-resize");
    expect(cursorFor("e", 90)).toBe("ns-resize");
    expect(cursorFor("ne", 0)).toBe("nesw-resize");
    expect(cursorFor("nw", 0)).toBe("nwse-resize");
    expect(cursorFor("rot", 0)).toBe("grab");
  });

  it("resizes from the opposite corner, from the centre, and evenly", () => {
    const r = resizeBox(box, "se", { x: 700, y: 500 }, { keepAspect: false, fromCenter: false });
    expect(r).toMatchObject({ x: 550, y: 425, w: 300, h: 150 });
    const c = resizeBox(box, "e", { x: 650, y: 999 }, { keepAspect: false, fromCenter: true });
    expect(c).toMatchObject({ x: 500, y: 400, w: 300, h: 100 });
    const even = resizeBox(box, "se", { x: 800, y: 460 }, { keepAspect: true, fromCenter: false });
    close(even.w / even.h, 2);
    close(even.w, 400);
    const side = resizeBox(box, "s", { x: 0, y: 550 }, { keepAspect: true, fromCenter: false });
    close(side.h, 200);
    close(side.w, 400);
  });

  it("never sizes below the minimum", () => {
    const r = resizeBox(box, "se", { x: 0, y: 0 }, { keepAspect: false, fromCenter: false });
    expect(r.w).toBeGreaterThanOrEqual(MIN_SIZE);
    expect(r.h).toBeGreaterThanOrEqual(MIN_SIZE);
  });

  it("resizes a turned box along its own sides", () => {
    // Turned a quarter, its east handle points down the canvas.
    const r = resizeBox(turned, "e", { x: 500, y: 600 }, { keepAspect: false, fromCenter: false });
    close(r.w, 300);
    close(r.x, 500);
    close(r.y, 450);
  });

  it("turns with the pointer and snaps to quarters and steps", () => {
    const b = { ...box, rotation: 0 };
    close(rotateBy(b, { x: 600, y: 400 }, { x: 500, y: 500 }, false), 90);
    close(rotateBy(b, { x: 600, y: 400 }, { x: 600, y: 403 }, false), 0);
    expect(rotateBy(b, { x: 600, y: 400 }, { x: 600, y: 440 }, true) % 15).toBe(0);
    expect(normAngle(270)).toBe(-90);
    expect(normAngle(-190)).toBe(170);
  });

  it("snaps a box to the nearest line and reports it", () => {
    const moving: Box = { x: 507, y: 300, w: 100, h: 100, rotation: 0 };
    const s = snap(moving, { xs: [512], ys: [] }, 6);
    expect(s.dx).toBe(5);
    expect(s.guides).toEqual([{ axis: "x", at: 512 }]);
    // An edge can snap too.
    const e = snap({ ...moving, x: 405 }, boxTargets([{ x: 300, y: 300, w: 10, h: 10, rotation: 0 }]), 6);
    expect(e.dx).toBe(0);
    const t = snap({ ...moving, x: 309 }, boxTargets([{ x: 250, y: 900, w: 10, h: 10, rotation: 0 }]), 6);
    expect(t.dx).toBe(-4);
    expect(snap(moving, { xs: [600], ys: [] }, 6).guides).toEqual([]);
  });
});
