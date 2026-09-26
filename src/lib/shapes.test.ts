import { describe, expect, it } from "vitest";
import { defaultShape, filterShapes, fold, groupShapes, matchScore, shapeName, shapeNote, shapeOf, type ShapeInfo } from "./shapes";

const shape = (id: string, family: string, system: string, label = id): ShapeInfo => ({ id, label, family, system, whole: family !== "free", thumbnail: null });
const SHAPES = [shape("mac-folder", "folder", "mac", "Mac folder"), shape("windows-folder", "folder", "windows", "Windows folder"), shape("free", "free", "any", "Free icon")];

describe("shapes", () => {
  it("start a chat on the folder the app puts skins on", () => {
    expect(defaultShape(SHAPES, "mac")?.id).toBe("mac-folder");
    expect(defaultShape(SHAPES, "windows")?.id).toBe("windows-folder");
    expect(shapeOf(SHAPES, undefined, "windows")?.id).toBe("windows-folder");
    expect(shapeOf(SHAPES, "free", "windows")?.id).toBe("free");
    // A chat saved with a shape this build doesn't have opens on the default.
    expect(shapeOf(SHAPES, "linux-drive", "mac")?.id).toBe("mac-folder");
    expect(defaultShape([], "mac")).toBeUndefined();
  });

  it("are named in the language on show, or by the app's own English when the catalog is new to one", () => {
    expect(shapeName(SHAPES[1])).toBe("Windows folder");
    expect(shapeNote(SHAPES[2])).toBe("A mascot, an object or a character, on its own");
    const drive = shape("mac-drive", "drive", "mac", "Mac drive");
    expect(shapeName(drive)).toBe("Mac drive");
    expect(shapeNote(drive)).toBe("");
  });

  it("group into folders, drives and the free icon, in the app's order within each", () => {
    const withDrives = [SHAPES[2], shape("mac-drive", "drive", "mac"), ...SHAPES.slice(0, 2)];
    expect(groupShapes(withDrives).map((g) => `${g.family}:${g.shapes.map((s) => s.id).join(",")}`)).toEqual([
      "folder:mac-folder,windows-folder",
      "drive:mac-drive",
      "free:free",
    ]);
  });

  it("are found by what's typed after @", () => {
    expect(filterShapes(SHAPES, "").map((s) => s.id)).toEqual(["mac-folder", "windows-folder", "free"]);
    expect(filterShapes(SHAPES, "win").map((s) => s.id)).toEqual(["windows-folder"]);
    expect(filterShapes(SHAPES, "FOLDER").map((s) => s.id)).toEqual(["mac-folder", "windows-folder"]);
    // By what it is, too: a mascot is a free icon.
    expect(filterShapes(SHAPES, "mascot").map((s) => s.id)).toEqual(["free"]);
    expect(filterShapes(SHAPES, "zzz")).toEqual([]);
  });

  it("match what's typed however it's accented or capitalised", () => {
    expect(fold("Écran Ünï")).toBe("ecran uni");
    expect(matchScore("mac", "Mac folder")).toBe(3);
    expect(matchScore("fold", "Mac folder")).toBe(2);
    expect(matchScore("finder", "Mac folder", "as Finder shows it")).toBe(1);
    expect(matchScore("drive", "Mac folder")).toBe(0);
  });
});
