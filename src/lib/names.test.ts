import { describe, expect, it } from "vitest";
import { cleanName, MAX_NAME_CHARS } from "./names";

const BELL = String.fromCharCode(7);

describe("skin names", () => {
  it("keeps a name on one line with single spaces", () => {
    expect(cleanName("  Beach   trip\n2026 ")).toBe("Beach trip 2026");
    expect(cleanName(`x${BELL}y`)).toBe("xy");
  });

  it("is empty when only white space was typed", () => {
    expect(cleanName(" \t\n ")).toBe("");
  });

  it("cuts long names between characters, like the store does", () => {
    expect(Array.from(cleanName("🦊".repeat(80)))).toHaveLength(MAX_NAME_CHARS);
    expect(cleanName(`${"a".repeat(59)} tail`)).toBe("a".repeat(59));
  });
});
