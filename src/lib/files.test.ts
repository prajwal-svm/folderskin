import { describe, expect, it } from "vitest";
import { baseName, dragInfoFor, isImagePath, looksLikeDrive, prettyPath } from "./files";

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
    expect(dragInfoFor(["/Volumes/Backup Disk"])).toEqual({ kind: "drive", name: "Backup Disk" });
  });

  it("guesses a drive's root from where it's mounted", () => {
    for (const path of ["/", "/Volumes/Backup Disk", "/Volumes/Backup Disk/", "E:\\", "e:", "/media/me/STICK", "/run/media/me/STICK"]) {
      expect(looksLikeDrive(path), path).toBe(true);
    }
    for (const path of ["/Volumes/Backup Disk/Photos", "/Volumes", "E:\\Photos", "/media/me", "/Users/me/Desktop", "/mnt"]) {
      expect(looksLikeDrive(path), path).toBe(false);
    }
  });

  it("shows the home folder as ~", () => {
    expect(prettyPath("/Users/me/Desktop/readme")).toBe("~/Desktop/readme");
    expect(prettyPath("/home/me")).toBe("~");
    expect(prettyPath("/Volumes/Backup/Photos")).toBe("/Volumes/Backup/Photos");
    expect(prettyPath("C:\\Users\\me")).toBe("C:\\Users\\me");
  });
});
