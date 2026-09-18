import { describe, expect, it } from "vitest";
import { baseName, dragInfoFor, isImagePath, prettyPath } from "./files";

describe("file helpers", () => {
  it("names the last path component on every OS", () => {
    expect(baseName("/Users/me/Desktop/Projects")).toBe("Projects");
    expect(baseName("/Users/me/Desktop/Projects/")).toBe("Projects");
    expect(baseName("C:\\Users\\me\\Pictures\\cat.JPG")).toBe("cat.JPG");
  });

  it("recognises pictures by extension, case-insensitively", () => {
    expect(isImagePath("/tmp/cat.HEIC")).toBe(true);
    expect(isImagePath("/tmp/poster.webp")).toBe(true);
    expect(isImagePath("/tmp/notes.txt")).toBe(false);
    expect(isImagePath("/tmp/.png")).toBe(false);
    expect(isImagePath("/tmp/Screenshots")).toBe(false);
  });

  it("guesses what a drag carries from its first path", () => {
    expect(dragInfoFor([])).toBeNull();
    expect(dragInfoFor(["/Users/me/Desktop/readme"])).toEqual({ kind: "folder", name: "readme" });
    expect(dragInfoFor(["/Users/me/art.png", "/Users/me/Desktop"])).toEqual({ kind: "image", name: "art.png" });
  });

  it("shows the home folder as ~", () => {
    expect(prettyPath("/Users/me/Desktop/readme")).toBe("~/Desktop/readme");
    expect(prettyPath("/home/me")).toBe("~");
    expect(prettyPath("/Volumes/Backup/Photos")).toBe("/Volumes/Backup/Photos");
    expect(prettyPath("C:\\Users\\me")).toBe("C:\\Users\\me");
  });
});
