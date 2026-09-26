import { describe, expect, it } from "vitest";
import { drawnOnDrive, driveKindName, driveName, shapeFirst, type Drive } from "./drives";

const drive = (patch: Partial<Drive>): Drive => ({
  kind: "external",
  shape: "mac-external",
  label: "Backup Disk",
  letter: null,
  startup: false,
  read_only: false,
  network: false,
  locked: null,
  thumbnails: "fsdrive://localhost/v3/mac-external/",
  plain: "data:image/png;base64,",
  ...patch,
});

describe("drives", () => {
  it("says what kind of drive was picked", () => {
    expect(driveKindName(drive({}))).toBe("External drive");
    expect(driveKindName(drive({ kind: "network" }))).toBe("Network drive");
    expect(driveKindName(drive({ kind: "removable" }))).toBe("USB drive");
    expect(driveKindName(drive({ kind: "startup" }))).toBe("Startup disk");
    // Windows calls the drive it runs from the system drive.
    expect(driveKindName(drive({ kind: "startup", letter: "C" }))).toBe("System drive");
  });

  it("names a drive as its system does", () => {
    expect(driveName(drive({}))).toBe("Backup Disk");
    expect(driveName(drive({ label: "Backup", letter: "E" }))).toBe("Backup (E:)");
    expect(driveName(drive({ kind: "removable", label: "", letter: "F" }))).toBe("USB drive (F:)");
    expect(driveName(drive({ kind: "startup", label: "" }))).toBe("Startup disk");
  });

  it("puts drive skins first for a drive and last for a folder, each keeping its order", () => {
    const skins = [{ id: "a", shape: "folder" as const }, { id: "b", shape: "drive" as const }, { id: "c" }, { id: "d", shape: "drive" as const }];
    expect(shapeFirst(skins, true).map((s) => s.id)).toEqual(["b", "d", "a", "c"]);
    expect(shapeFirst(skins, false).map((s) => s.id)).toEqual(["a", "c", "b", "d"]);
    // Nothing to move, nothing moves.
    const folders = skins.filter((s) => s.shape !== "drive");
    expect(shapeFirst(folders, true)).toBe(folders);
  });

  it("draws only artwork again for the drive it goes on", () => {
    expect(drawnOnDrive({ kind: "artwork" })).toBe(true);
    expect(drawnOnDrive({ kind: "folder" })).toBe(false);
    expect(drawnOnDrive({})).toBe(false);
  });
});
