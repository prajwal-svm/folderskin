import { describe, expect, it } from "vitest";
import { duration, machineDetails, whatItTakes } from "./localSetup";

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
    expect(whatItTakes({ download_bytes: 0, installs: "mflux" })).toBe("Installs mflux; the model is already here.");
    expect(whatItTakes({ download_bytes: 0, installs: null })).toBe("Nothing left to download; setting up checks what's here.");
  });
});

describe("the details about your machine", () => {
  const status = { backend: "MLX", seconds_per_image: 46.2, home: "/Users/you/Library/Caches/folderskin-localgen" };

  it("says how long the last picture took here, and where the files are, a line each", () => {
    expect(machineDetails(status).split("\n")).toEqual([
      "Runs with MLX",
      "The last picture here took about 46 seconds",
      "Kept in /Users/you/Library/Caches/folderskin-localgen",
    ]);
  });

  it("says the time comes after the first picture, before there is one", () => {
    expect(machineDetails({ ...status, seconds_per_image: null })).toContain("after the first one");
  });

  it("counts a long time in minutes", () => {
    expect(duration(59)).toBe("59 seconds");
    expect(duration(150)).toBe("3 minutes");
  });
});
