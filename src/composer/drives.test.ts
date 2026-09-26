import { readFileSync, readdirSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { defaultDrive, DRIVE_FACES, DRIVE_IDS, DRIVE_KINDS, driveKindOf, driveLabel, driveParts, driveStyleOf, isDriveId } from "./drives";
import english from "../locales/en/common.json";

/** What `folderskin-tools composer-layers` wrote from the Rust drawing, beside the drives' layers. */
const written = JSON.parse(readFileSync(new URL("../../docs/images/composer/drives/parts.json", import.meta.url), "utf8")) as Record<
  string,
  { face: number[]; point: number[]; extent: number[] }
>;

describe("the drives", () => {
  it("are the ones Rust draws, with their faces where Rust draws them", () => {
    expect([...DRIVE_IDS].sort()).toEqual(Object.keys(written).sort());
    for (const id of DRIVE_IDS) {
      const ours = DRIVE_FACES[id];
      for (const part of ["face", "point", "extent"] as const) {
        ours[part].forEach((v, i) => expect(v, `${id} ${part}`).toBeCloseTo(written[id][part][i], 1));
      }
    }
    // Each has its layers and its bare shape in the browser preview's pictures too.
    const bases = readdirSync(new URL("../../docs/images/composer/bases", import.meta.url));
    for (const id of DRIVE_IDS) expect(bases, id).toContain(`${id}.webp`);
  });

  it("each have a short name, and a full name and a note as a shape, in English", () => {
    const short = english.driveNames as Record<string, string>;
    const full = english.shapes as Record<string, string>;
    const notes = english.shapeNotes as Record<string, string>;
    for (const id of DRIVE_IDS) {
      expect(short[id], id).toBeTruthy();
      expect(full[id], id).toContain(short[id].slice(1));
      expect(notes[id], id).toBeTruthy();
    }
    expect(driveLabel("linux-removable")).toBe("USB stick");
  });

  it("are known by their ids and nothing else", () => {
    expect(DRIVE_IDS).toHaveLength(9 + 6 + 10);
    expect(isDriveId("linux-solid-state")).toBe(true);
    expect(isDriveId("mac-solid-state")).toBe(false);
    expect(isDriveId(7)).toBe(false);
    expect([driveStyleOf("linux-solid-state"), driveKindOf("linux-solid-state")]).toEqual(["linux", "solid-state"]);
    expect(DRIVE_KINDS.windows).toContain("startup");
  });

  it("start a design on each system's own drive", () => {
    expect(defaultDrive("macos")).toBe("mac-external");
    expect(defaultDrive("windows")).toBe("windows-internal");
    expect(defaultDrive("linux")).toBe("linux-external");
  });

  it("place new layers on the face, and on a disc where it shows", () => {
    const box = driveParts("mac-external");
    expect(box.front).toEqual(DRIVE_FACES["mac-external"].face);
    expect(box.tab).toEqual(box.front);
    expect(box.anchor).toBeNull();
    const disc = driveParts("mac-optical");
    expect(disc.anchor).toEqual(DRIVE_FACES["mac-optical"].point);
    expect(disc.anchor![1]).toBeLessThan(400);
  });
});
