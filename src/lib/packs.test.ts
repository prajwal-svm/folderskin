import { describe, expect, it } from "vitest";
import { isGithubUser, licenseLabel, packSlug } from "./packs";

describe("packs", () => {
  // The same cases as `ids_file_names_and_users_cannot_escape_their_url` in pack.rs.
  it("take a GitHub user name as the author", () => {
    expect(isGithubUser("prajwal-svm")).toBe(true);
    for (const bad of ["", "-x", "x-", "a--b", "a b", "a".repeat(40)]) expect(isGithubUser(bad), bad).toBe(false);
  });

  it("get their folder name from the pack name", () => {
    expect(packSlug("Ukiyo-e Nights!")).toBe("ukiyo-e-nights");
    expect(packSlug("  ***  ")).toBe("");
    expect(packSlug("Con")).toBe("con-1");
    expect(packSlug("Long name ".repeat(10)).length).toBeLessThanOrEqual(40);
  });

  it("name their licence the short way", () => {
    expect(licenseLabel("CC-BY-4.0")).toBe("CC BY 4.0");
    expect(licenseLabel("CC0-1.0")).toBe("CC0");
    expect(licenseLabel("MIT")).toBe("MIT");
  });
});
