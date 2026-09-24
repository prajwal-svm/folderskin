import { describe, expect, it } from "vitest";
import { cleanName, clip, MAX_NAME_CHARS } from "./names";

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

  it("cuts a long name short inside a sentence, after a whole word when it can, with no dots", () => {
    expect(clip("Beach trip")).toBe("Beach trip");
    expect(clip("Grandma's lighthouse at dusk on the northern coast, summer")).toBe("Grandma's lighthouse at dusk on");
    expect(clip("Northwind Traders rebrand, final deliverables")).toBe("Northwind Traders rebrand, final");
    expect(clip("Northwind Traders, rebrand", 20)).toBe("Northwind Traders");
    expect(Array.from(clip("🦊".repeat(40)))).toHaveLength(32);
    for (const name of ["Grandma's lighthouse at dusk on the northern coast, summer", "🦊".repeat(40), `${"a".repeat(50)} b`]) {
      expect(clip(name)).not.toMatch(/…|\.\.\.$/);
    }
  });
});
