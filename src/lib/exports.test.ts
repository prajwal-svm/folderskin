import { describe, expect, it } from "vitest";
import { exportKinds, isFile, suggestedName, withEnding } from "./exports";

describe("exporting a skin", () => {
  it("offers this system's own icon file first, and every kind once", () => {
    expect(exportKinds("macos")[0]).toBe("icns");
    expect(exportKinds("windows")[0]).toBe("ico");
    expect(exportKinds("linux")[0]).toBe("png");
    for (const os of ["macos", "windows", "linux"] as const) expect(new Set(exportKinds(os)).size).toBe(8);
  });

  it("suggests a name that ends as the kind does", () => {
    expect(suggestedName("Mona Lisa", "icns")).toBe("Mona Lisa.icns");
    expect(suggestedName("Mona Lisa", "jpeg")).toBe("Mona Lisa.jpg");
    expect(suggestedName("Mona Lisa", "ios")).toBe("Mona Lisa.appiconset");
    expect(suggestedName("Mona Lisa", "favicon")).toBe("Mona Lisa favicon");
    expect(suggestedName("Mona Lisa", "folder")).toBe("Mona Lisa");
  });

  it("adds the ending a name was typed without, and only then", () => {
    expect(withEnding("/a/Mona", "icns")).toBe("/a/Mona.icns");
    expect(withEnding("/a/Mona.ICNS", "icns")).toBe("/a/Mona.ICNS");
    expect(withEnding("/a/Mona.jpeg", "jpeg")).toBe("/a/Mona.jpeg");
    expect(withEnding("/a/Mona", "iconset")).toBe("/a/Mona.iconset");
    expect(withEnding("/a/site", "favicon")).toBe("/a/site");
    expect(withEnding("/a/Mona", "folder")).toBe("/a/Mona");
  });

  it("knows which kinds are one file", () => {
    expect(["icns", "ico", "png", "jpeg"].every((k) => isFile(k as never))).toBe(true);
    expect(isFile("iconset") || isFile("ios") || isFile("favicon") || isFile("folder")).toBe(false);
  });
});
