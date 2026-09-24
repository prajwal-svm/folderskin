import { describe, expect, it } from "vitest";
import { placeTip, TIP_GAP, TIP_MARGIN } from "./tip";

const win = { w: 1000, h: 700 };
const tip = { w: 120, h: 28 };
const button = (left: number, top: number, w = 30, h = 30) => ({ left, top, right: left + w, bottom: top + h });

describe("placeTip", () => {
  it("sits above its element, centred on it", () => {
    const p = placeTip(button(400, 300), tip, win, "top");
    expect(p).toEqual({ x: 415 - 60, y: 300 - TIP_GAP - 28, side: "top" });
  });

  it("goes below when there is no room above", () => {
    const p = placeTip(button(400, 10), tip, win, "top");
    expect(p.side).toBe("bottom");
    expect(p.y).toBe(40 + TIP_GAP);
  });

  it("goes to the left of something at the right edge that prefers the right", () => {
    const p = placeTip(button(960, 300), tip, win, "right");
    expect(p.side).toBe("left");
    expect(p.x).toBe(960 - TIP_GAP - 120);
  });

  it("slides along to stay inside the window", () => {
    expect(placeTip(button(0, 300), tip, win, "top").x).toBe(TIP_MARGIN);
    expect(placeTip(button(990, 300, 10), tip, win, "top").x).toBe(win.w - TIP_MARGIN - 120);
  });

  it("keeps its side when neither side has room", () => {
    const tall = { w: 120, h: 400 };
    expect(placeTip(button(400, 300), tall, win, "top").side).toBe("top");
  });
});
