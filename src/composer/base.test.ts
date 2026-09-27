import { describe, expect, it } from "vitest";
import { applyMatrix, BASE_ID, baseSelection, framedBox, frameMatrix, invertMatrix, partLabel, partSelection, partsOf, type Piece, type PieceRole } from "./base";

const piece = (part: string, role: PieceRole): Piece => ({ part, role, rect: [0, 0, 1, 1], img: {} as HTMLImageElement });

describe("a folder's or drive's parts", () => {
  it("are listed top first, each where it shows highest, its light and shade not counting", () => {
    // The USB stick as Rust takes it apart: under the face, the face, then the face's rims.
    const stick = [piece("port", "paint"), piece("holes", "paint"), piece("case", "paint"), piece("face", "paint"), piece("face", "surface"), piece("face", "light")];
    expect(partsOf(stick).map((p) => p.id)).toEqual(["face", "case", "holes", "port"]);
    expect(partsOf(stick)[0]).toEqual({ id: "face", surface: true, paints: true });
    // The Mac's folder: its tab is the back panel's rows above its body, listed under the back,
    // and the front's light over everything doesn't lift the front above where it shows.
    const tab = { part: "tab", row: 194 };
    const folder = [
      { ...piece("back", "surface"), split: tab },
      { ...piece("back", "light"), split: tab },
      piece("paper", "paint"),
      piece("front", "surface"),
      piece("front", "light"),
    ];
    expect(partsOf(folder).map((p) => p.id)).toEqual(["front", "paper", "back", "tab"]);
    expect(partsOf(folder).find((p) => p.id === "tab")).toEqual({ id: "tab", surface: true, paints: false });
    expect(partsOf(folder).find((p) => p.id === "paper")).toEqual({ id: "paper", surface: false, paints: true });
    // A slab over a disc, drawn only over the face, is over it in the list too.
    const discDrive = [piece("disc", "paint"), piece("face", "paint"), piece("face", "surface"), piece("disc", "light"), piece("case", "paint")];
    expect(partsOf(discDrive).map((p) => p.id)).toEqual(["case", "face", "disc"]);
  });

  it("are named for the list, and one FolderSkin has no name for yet by its id", () => {
    expect(partLabel("holes")).toBe("Port holes");
    expect(partLabel("back-disk")).toBe("Back disk");
    expect(partLabel("new-thing")).toBe("New thing");
  });

  it("are selected by ids no layer can have", () => {
    expect(baseSelection(BASE_ID)).toEqual({ part: null });
    expect(baseSelection(partSelection("case"))).toEqual({ part: "case" });
    expect(baseSelection("l1abc")).toBeNull();
    expect(baseSelection(null)).toBeNull();
  });
});

describe("the frame", () => {
  const pivot = { x: 512, y: 522 };
  const close = (a: { x: number; y: number }, b: { x: number; y: number }) => {
    expect(a.x).toBeCloseTo(b.x, 6);
    expect(a.y).toBeCloseTo(b.y, 6);
  };

  it("leaves everything where it is at home", () => {
    const m = frameMatrix({ x: 0, y: 0, rotation: 0, scale: 1 }, pivot);
    close(applyMatrix(m, { x: 100, y: 900 }), { x: 100, y: 900 });
  });

  it("turns and sizes about the middle, then moves", () => {
    const m = frameMatrix({ x: 10, y: -20, rotation: 90, scale: 2 }, pivot);
    // The middle only moves.
    close(applyMatrix(m, pivot), { x: 522, y: 502 });
    // A point above the middle turns clockwise to its right, twice as far out.
    close(applyMatrix(m, { x: 512, y: 422 }), { x: 512 + 200 + 10, y: 522 - 20 });
    // And the inverse brings it back.
    close(applyMatrix(invertMatrix(m), applyMatrix(m, { x: 300, y: 700 })), { x: 300, y: 700 });
  });

  it("takes a layer's box with it", () => {
    const b = framedBox({ x: 512, y: 422, w: 100, h: 50, rotation: 10 }, { x: 0, y: 0, rotation: 90, scale: 0.5 }, pivot);
    expect(b.x).toBeCloseTo(562, 6);
    expect(b.y).toBeCloseTo(522, 6);
    expect([b.w, b.h, b.rotation]).toEqual([50, 25, 100]);
  });
});
