import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { makeIcon, parseDoc } from "../doc";
import { drawingOf, indexPack, parsePack, searchIcons, type IconPack } from "./index";
import { ICON_PACKS } from "./catalog";

const lucide = parsePack(readFileSync(new URL("./lucide.json", import.meta.url), "utf8")) as IconPack;

describe("the built-in pack", () => {
  it("reads as a pack of line icons with tags", () => {
    expect(lucide).not.toBeNull();
    expect(lucide.id).toBe("lucide");
    expect(lucide.style).toBe("stroke");
    expect(lucide.viewBox).toBe(24);
    expect(lucide.icons.length).toBeGreaterThan(1500);
    const camera = lucide.icons.find((i) => i.n === "camera");
    expect(camera?.d.length).toBeGreaterThan(0);
    expect(camera?.t).toContain("photography");
  });

  it("matches the catalog the app downloads packs by", () => {
    const entry = ICON_PACKS.find((p) => p.id === "lucide");
    expect(entry?.builtin).toBe(true);
    expect(entry?.count).toBe(lucide.icons.length);
    // Every downloadable pack carries the hash its download is checked against.
    for (const p of ICON_PACKS.filter((x) => !x.builtin)) expect(p.sha256).toMatch(/^[0-9a-f]{64}$/);
  });
});

describe("searching", () => {
  const index = indexPack(lucide);

  it("puts the icon of that name first", () => {
    expect(searchIcons(lucide, index, "camera")[0].n).toBe("camera");
    expect(searchIcons(lucide, index, "Folder")[0].n).toBe("folder");
  });

  it("finds icons by the words people use, and needs every word", () => {
    const photo = searchIcons(lucide, index, "photography").map((i) => i.n);
    expect(photo).toContain("camera");
    const both = searchIcons(lucide, index, "arrow up").map((i) => i.n);
    expect(both[0]).toBe("arrow-up");
    // Every result has both words, in its name or its search words.
    const index2 = indexPack(lucide);
    for (const n of both) {
      const i = lucide.icons.findIndex((x) => x.n === n);
      const all = `${index2.names[i]} ${index2.words[i]}`;
      expect(all.includes("arrow") && all.includes("up")).toBe(true);
    }
    expect(searchIcons(lucide, index, "zzqqxx")).toEqual([]);
  });

  it("shows the whole pack for an empty search", () => {
    expect(searchIcons(lucide, index, "  ")).toHaveLength(lucide.icons.length);
  });
});

describe("reading packs and icon layers", () => {
  it("refuses what isn't a pack and leaves out icons that aren't path data", () => {
    expect(parsePack("not json")).toBeNull();
    expect(parsePack({ format: 2, id: "x", name: "X", icons: [] })).toBeNull();
    const p = parsePack({ format: 1, id: "x", name: "X", style: "fill", viewBox: 16, icons: [{ n: "ok", d: ["M0 0h16v16H0z"] }, { n: "bad", d: ["<script>"] }, { n: "empty", d: [] }] });
    expect(p?.icons.map((i) => i.n)).toEqual(["ok"]);
    expect(p?.style).toBe("fill");
  });

  it("keeps an icon's drawing in the design, so it opens without the pack", () => {
    const camera = lucide.icons.find((i) => i.n === "camera")!;
    const layer = makeIcon(drawingOf(lucide, camera), 512, 560);
    const doc = parseDoc({ version: 1, shape: "folder", layers: [layer] });
    const back = doc?.layers[0];
    expect(back?.kind).toBe("icon");
    if (back?.kind !== "icon") return;
    expect(back.paths).toEqual(camera.d);
    expect(back.look).toBe("emboss");
    expect(back.auto).toBe(true);
    // Anything but path data is dropped, and a layer left with no drawing is dropped with it.
    const bad = parseDoc({ version: 1, shape: "folder", layers: [{ ...layer, paths: ['M0 0"/><script>'] }] });
    expect(bad?.layers).toEqual([]);
  });
});
