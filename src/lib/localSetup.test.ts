import { describe, expect, it } from "vitest";
import { duration, spaceShort, whatItTakes } from "./localSetup";

describe("what setting the local model up takes", () => {
  it("says a Mac installs mflux and downloads the model, with its size", () => {
    expect(whatItTakes({ download_bytes: 4_619_699_678, installs: "mflux" })).toBe(
      "Installs mflux and downloads 4.6 GB once, then works offline.",
    );
  });

  it("gives the download alone where the runtime comes in it", () => {
    expect(whatItTakes({ download_bytes: 5_207_178_964, installs: null })).toBe("Downloads 5.2 GB once, then works offline.");
  });

  it("says what is left when only one part is", () => {
    expect(whatItTakes({ download_bytes: 0, installs: "mflux" })).toBe("Installs mflux. The model is already here.");
    expect(whatItTakes({ download_bytes: 0, installs: null })).toBe("Nothing left to download. Setting up checks what's here.");
  });
});

describe("how long a picture took", () => {
  it("counts a long time in minutes", () => {
    expect(duration(46.2)).toBe("46 seconds");
    expect(duration(59)).toBe("59 seconds");
    expect(duration(150)).toBe("3 minutes");
  });
});

describe("room on the disk", () => {
  it("asks for space to be cleared below half again what setting up puts there", () => {
    expect(spaceShort({ free_bytes: 3_200_000_000, wanted_bytes: 6_930_000_000 })).toBe(
      "Clear some space first: setting it up wants 6.9 GB free, and this disk has 3.2 GB.",
    );
  });

  it("says nothing when there's room, nothing to set up, or the system didn't say", () => {
    expect(spaceShort({ free_bytes: 90_000_000_000, wanted_bytes: 6_930_000_000 })).toBeNull();
    expect(spaceShort({ free_bytes: 1_000, wanted_bytes: 0 })).toBeNull();
    expect(spaceShort({ free_bytes: null, wanted_bytes: 6_930_000_000 })).toBeNull();
  });
});
