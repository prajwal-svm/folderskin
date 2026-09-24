import { describe, expect, it } from "vitest";
import { whatItTakes } from "./localSetup";

describe("what setting this computer up takes", () => {
  it("says a Mac installs mflux and downloads the model, with its size", () => {
    expect(whatItTakes({ download_bytes: 4_619_699_678, installs: "mflux" })).toBe(
      "Installs mflux and downloads 4.6 GB once, then works offline.",
    );
  });

  it("gives the download alone where the runtime comes in it", () => {
    expect(whatItTakes({ download_bytes: 5_207_178_964, installs: null })).toBe("Downloads 5.2 GB once, then works offline.");
  });

  it("says what is left when only one part is", () => {
    expect(whatItTakes({ download_bytes: 0, installs: "mflux" })).toBe("Installs mflux; the model is already here.");
    expect(whatItTakes({ download_bytes: 0, installs: null })).toBe("Nothing left to download; setting up checks what's here.");
  });
});
