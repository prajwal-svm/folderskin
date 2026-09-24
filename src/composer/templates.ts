/**
 * Starting points for a new design, from a plain folder to a finished-looking one. Each is an
 * ordinary document the user can change in every way; `photo` ones start by asking for a picture.
 */
import { FOLDER_BLUE_BOTTOM, FOLDER_BLUE_TOP } from "./color";
import {
  centreOf,
  emptyDoc,
  imageBox,
  linear,
  makeEmoji,
  makeFill,
  makeImage,
  makePattern,
  makeShape,
  makeText,
  solid,
  type Doc,
  type Layer,
  type Parts,
} from "./doc";

export type Picture = { src: string; width: number; height: number; alpha: boolean };

export type Template = {
  id: string;
  label: string;
  /** Needs a picture before it can start. */
  photo?: boolean;
  make: (parts: Parts, picture?: Picture) => Doc;
};

const doc = (layers: Layer[], shape: Doc["shape"] = "folder"): Doc => ({ ...emptyDoc(shape), layers });

export const TEMPLATES: Template[] = [
  {
    id: "plain",
    label: "Plain",
    make: () => doc([makeFill(linear(180, FOLDER_BLUE_TOP, FOLDER_BLUE_BOTTOM))]),
  },
  {
    id: "colour",
    label: "Colour",
    make: () => doc([makeFill(solid("#ff7a59"))]),
  },
  {
    id: "label",
    label: "Label",
    make: (p) => {
      const c = centreOf(p.front);
      return doc([makeFill(linear(180, "#2b4c7e", "#1c3257")), { ...makeText("Projects", c.x, c.y, "#ffffff"), size: 150 }]);
    },
  },
  {
    id: "emoji",
    label: "Emoji",
    make: (p) => {
      const c = centreOf(p.front);
      return doc([makeFill(linear(160, "#ffd3a5", "#fd6585")), { ...makeEmoji("📸", c.x, c.y + 6), size: 400 }]);
    },
  },
  {
    id: "tab",
    label: "Tab label",
    make: (p) => {
      const c = centreOf(p.front);
      const t = centreOf(p.tab);
      return doc([
        makeFill(linear(150, "#8f5cff", "#5b2bd9")),
        { ...makeText("IDEAS", t.x, t.y + 3, "#ffffff"), size: 44, weight: 900, spacing: 0.08, font: "system" },
        { ...makeEmoji("💡", c.x, c.y + 6), size: 380 },
      ]);
    },
  },
  {
    id: "two-tone",
    label: "Two-tone",
    make: (p) => {
      // The split sits behind the paper sheet, so the back is one colour and the front another.
      // Windows' folder has no sheet (its `paper` is the back's body, as wide as the back), so
      // there the split is the front's top edge: right of the tab exactly, and under the tab,
      // where the front's edge steps lower, a strip of the back takes the front's colour.
      const sheet = p.paper[0] > p.back[0];
      const split = sheet ? (p.paper[1] + p.front[1]) / 2 : p.front[1];
      const t = centreOf(p.tab);
      return doc([
        makeFill(solid("#ffb703")),
        { ...makeShape("rect", 512, (split + 1100) / 2, "#023047"), w: 1100, h: 1100 - split, radius: 0, name: "Front" },
        { ...makeText("WORK", t.x, t.y + 3, "#023047"), size: 46, weight: 900, spacing: 0.1, font: "system" },
      ]);
    },
  },
  {
    id: "glass",
    label: "Glass",
    make: (p) => {
      const c = centreOf(p.front);
      return doc([
        makeFill(solid("#ffffff2e")),
        { ...makeFill(linear(180, "#ffffff66", "#ffffff00", "#ffffff00")), name: "Shine" },
        { ...makeEmoji("💎", c.x, c.y + 6), size: 340 },
      ]);
    },
  },
  {
    id: "tinted",
    label: "Tinted glass",
    make: (p) => {
      const c = centreOf(p.front);
      return doc([makeFill(solid("#3a86ff70")), { ...makeText("Drafts", c.x, c.y, "#ffffff"), size: 140, opacity: 0.9 }]);
    },
  },
  {
    id: "stripes",
    label: "Stripes",
    make: (p) => {
      const c = centreOf(p.front);
      return doc([
        makeFill(solid("#ff5a5f")),
        { ...makePattern("stripes", "#ffffff4d"), scale: 76, angle: 45 },
        { ...makeShape("rect", c.x, c.y, "#ffffff"), w: 560, h: 210, radius: 1, name: "Label" },
        { ...makeText("Notes", c.x, c.y, "#ff5a5f"), size: 120 },
      ]);
    },
  },
  {
    id: "gingham",
    label: "Gingham",
    make: (p) => {
      const c = centreOf(p.front);
      return doc([
        makeFill(solid("#fffaf5")),
        { ...makePattern("gingham", "#e6394666"), scale: 92 },
        { ...makeEmoji("🍓", c.x, c.y + 6), size: 330, edge: { color: "#ffffff", width: 18 } },
      ]);
    },
  },
  {
    id: "polka",
    label: "Polka",
    make: () => doc([makeFill(solid("#1d3557")), { ...makePattern("dots", "#f1faeecc"), scale: 96 }]),
  },
  {
    id: "sunset",
    label: "Sunset",
    make: () =>
      doc([makeFill(linear(180, "#fbd07c", "#f7797d", "#6e48aa")), { ...makePattern("grain", "#ffffff"), scale: 2, opacity: 0.35, name: "Grain" }]),
  },
  {
    id: "neon",
    label: "Neon",
    make: (p) => {
      const c = centreOf(p.front);
      return doc([
        makeFill(linear(180, "#15102b", "#0a0818")),
        {
          ...makeText("Night", c.x, c.y, "#ffd6f5"),
          size: 170,
          font: "script",
          weight: 700,
          stroke: { color: "#ff4fd8", width: 5 },
          shadow: { color: "#ff4fd8", blur: 46, x: 0, y: 0 },
        },
      ]);
    },
  },
  {
    id: "badge",
    label: "Badge",
    make: (p) => {
      const c = centreOf(p.front);
      return doc([
        makeFill(solid("#2a9d8f")),
        { ...makeShape("burst", c.x, c.y, "#e9c46a"), w: 470, h: 470, rotation: -8 },
        { ...makeText("NEW", c.x, c.y + 4, "#264653"), size: 132, font: "poster", weight: 400, rotation: -8 },
      ]);
    },
  },
  {
    id: "photo",
    label: "Photo",
    photo: true,
    make: (p, pic) => {
      const layers: Layer[] = [makeFill(solid("#20242c"))];
      if (pic) layers.push(makeImage(pic.src, pic.width, pic.height, imageBox(pic.width, pic.height, p, !pic.alpha)));
      const band = 150;
      const y = p.front[3] - band / 2 - 30;
      layers.push({ ...makeShape("rect", 512, y, "#00000066"), w: 1100, h: band, radius: 0, name: "Caption band" });
      layers.push({ ...makeText("Summer 2026", 512, y, "#ffffff"), size: 76, weight: 800 });
      return doc(layers);
    },
  },
  {
    id: "sticker",
    label: "Sticker",
    make: () =>
      doc(
        [{ ...makeEmoji("🐱", 512, 530), size: 720, edge: { color: "#ffffff", width: 30 }, shadow: { color: "#00000047", blur: 34, x: 0, y: 18 } }],
        "free",
      ),
  },
];

export const templateById = (id: string) => TEMPLATES.find((t) => t.id === id);
