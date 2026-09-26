import { describe, expect, it } from "vitest";
import { cursorFor } from "./cursor";

describe("cursorFor", () => {
  it("gives the hand for a pointer, and leaves auto to what's under the pointer", () => {
    expect(cursorFor("pointer")).toBe("hand");
    expect(cursorFor("default")).toBe("default");
    expect(cursorFor("auto")).toBeNull();
    expect(cursorFor("")).toBeNull();
  });
  it("reads the fallback after a custom image, drops WebKit's prefix and knows no others", () => {
    expect(cursorFor('url("data:image/png;base64,AAAA,BBBB") 4 4, pointer')).toBe("hand");
    expect(cursorFor("-webkit-grab")).toBe("grab");
    expect(cursorFor("none")).toBe("default");
    expect(cursorFor("something-new")).toBe("default");
  });
  it("keeps every other cursor the app uses", () => {
    expect(cursorFor("text")).toBe("text");
    expect(cursorFor("not-allowed")).toBe("notAllowed");
    expect(cursorFor("col-resize")).toBe("colResize");
    expect(cursorFor("row-resize")).toBe("rowResize");
    expect(cursorFor("grab")).toBe("grab");
    expect(cursorFor("grabbing")).toBe("grabbing");
    expect(cursorFor(" progress ")).toBe("progress");
    expect(cursorFor("help")).toBe("help");
    expect(cursorFor("crosshair")).toBe("crosshair");
  });
  it("turns the composer's resize handles the right way", () => {
    expect(cursorFor("ew-resize")).toBe("ewResize");
    expect(cursorFor("ns-resize")).toBe("nsResize");
    expect(cursorFor("nesw-resize")).toBe("neswResize");
    expect(cursorFor("nwse-resize")).toBe("nwseResize");
  });
});
