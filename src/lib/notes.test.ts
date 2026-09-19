import { describe, expect, it } from "vitest";
import { parseNotes, splitInline } from "./notes";

describe("parseNotes", () => {
  it("reads a CHANGELOG section: headings, wrapped items and paragraphs", () => {
    const section = [
      "First release.",
      "",
      "### Added",
      "",
      "- Apply a folder icon on macOS, Windows and Linux from one rendering path in",
      "  `folderskin-core`, so the preview and the icon are the same pixels.",
      "- Community packs.",
      "",
      "### Fixed",
      "- A cut-out on grey keeps dark clothes.",
    ].join("\n");
    expect(parseNotes(section)).toEqual([
      { kind: "text", text: "First release." },
      { kind: "heading", text: "Added" },
      {
        kind: "list",
        items: [
          "Apply a folder icon on macOS, Windows and Linux from one rendering path in `folderskin-core`, so the preview and the icon are the same pixels.",
          "Community packs.",
        ],
      },
      { kind: "heading", text: "Fixed" },
      { kind: "list", items: ["A cut-out on grey keeps dark clothes."] },
    ]);
  });

  it("leaves out the downloads the release page adds after ---", () => {
    const body = "- One thing.\r\n\r\n---\r\n\r\n**Downloads:** a .dmg and an .msi.";
    expect(parseNotes(body)).toEqual([{ kind: "list", items: ["One thing."] }]);
  });

  it("joins a paragraph's lines, and a line that isn't indented ends a list", () => {
    expect(parseNotes("- An item\nNot part of it,\nstill the same paragraph.")).toEqual([
      { kind: "list", items: ["An item"] },
      { kind: "text", text: "Not part of it, still the same paragraph." },
    ]);
  });

  it("has nothing to show for empty notes", () => {
    expect(parseNotes("")).toEqual([]);
    expect(parseNotes("\n\n---\nfooter")).toEqual([]);
  });
});

describe("splitInline", () => {
  it("finds bold and code, and keeps a link's words", () => {
    expect(splitInline("A **Scientists** pack for `packs make`, see [the guide](https://example.com).")).toEqual([
      { kind: "text", text: "A " },
      { kind: "strong", text: "Scientists" },
      { kind: "text", text: " pack for " },
      { kind: "code", text: "packs make" },
      { kind: "text", text: ", see " },
      { kind: "text", text: "the guide" },
      { kind: "text", text: "." },
    ]);
  });

  it("leaves a plain line alone", () => {
    expect(splitInline("Nothing special")).toEqual([{ kind: "text", text: "Nothing special" }]);
    expect(splitInline("")).toEqual([]);
  });
});
