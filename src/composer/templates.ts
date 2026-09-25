/**
 * Starting points for a new design, from a plain folder to a finished-looking one. Each is an
 * ordinary document the user can change in every way; `photo` ones start by asking for a picture.
 */
import "../i18n/composer";
import { t } from "../i18n";
import { FOLDER_BLUE_BOTTOM, FOLDER_BLUE_TOP } from "./color";
import {
  CANVAS,
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
  type FolderStyle,
  type Layer,
  type Paint,
  type Parts,
} from "./doc";

export type Picture = { src: string; width: number; height: number; alpha: boolean };

export type Template = {
  /** Named by `composer.templates.<id>`. */
  id: string;
  /** Starts on this folder whichever folder the last design was on: it's that folder's own look. */
  style?: FolderStyle;
  /** A folder as its system draws it, with nothing on it: offered with the empty starts, not the
   *  templates, with `composer.templateNotes.<id>` under its name. */
  plain?: boolean;
  /** Needs a picture before it can start. */
  photo?: boolean;
  /** The design, laid out on `parts`: those of `style`'s folder when the template has one. */
  make: (parts: Parts, picture?: Picture) => Doc;
};

const doc = (layers: Layer[], shape: Doc["shape"] = "folder", style: FolderStyle = "mac"): Doc => ({ ...emptyDoc(shape, style), layers });

/** A top-to-bottom gradient across the whole canvas, each colour at a height on it. */
const down = (...stops: [y: number, color: string][]): Paint => ({
  type: "linear",
  angle: 180,
  stops: stops.map(([y, color]) => ({ at: Math.min(1, Math.max(0, y / CANVAS)), color })),
});

export const TEMPLATES: Template[] = [
  {
    id: "plain",
    make: () => doc([makeFill(linear(180, FOLDER_BLUE_TOP, FOLDER_BLUE_BOTTOM))]),
  },
  {
    // The folder Finder draws: a deeper blue back and tab, a lighter front that darkens along
    // the bottom, with the pocket's seam across it. Colours from macOS's own folder.
    id: "mac",
    style: "mac",
    plain: true,
    make: (p) => {
      const back = (share: number) => p.back[1] + share * (p.front[1] - p.back[1]);
      const front = (share: number) => p.front[1] + share * (p.front[3] - p.front[1]);
      return doc(
        [
          { ...makeFill(down([back(0.08), "#3aaee4"], [back(0.38), "#35ace1"], [back(0.62), "#1aa2dd"], [back(0.87), "#008bce"])), name: t("composer.templateLayers.back") },
          {
            ...makeFill(
              down(
                [front(0.02), "#56c9f9"],
                [front(0.1), "#60d0ff"],
                [front(0.79), "#5fcffe"],
                [front(0.84), "#56cafa"],
                [front(0.865), "#5fcdf8"],
                [front(0.89), "#46bff1"],
                [front(0.94), "#34ade6"],
                [front(1), "#1ca1dd"],
              ),
            ),
            part: "front",
            name: t("composer.templateLayers.front"),
          },
        ],
        "folder",
        "mac",
      );
    },
  },
  {
    // The folder Explorer draws: a golden back and tab (the folder darkens its back a little
    // more itself), a pale yellow front deepening towards its bottom right, and a deeper lip
    // along its bottom. Colours from Windows 11's own folder.
    id: "windows",
    style: "windows",
    plain: true,
    make: (p) => {
      // Under the tab the front's top edge is 48 units lower than elsewhere: the back shows to there.
      const lowest = p.front[1] + 48;
      const lip = 17;
      const bottom = p.front[3] + 10;
      return doc(
        [
          { ...makeFill(down([p.back[1], "#ffca1e"], [lowest, "#f6b919"])), name: t("composer.templateLayers.back") },
          {
            ...makeFill({ type: "linear", angle: 125, stops: [{ at: 0.19, color: "#ffe59a" }, { at: 0.85, color: "#ffcb3d" }] }),
            part: "front",
            name: t("composer.templateLayers.front"),
          },
          {
            ...makeShape("rect", (p.front[0] + p.front[2]) / 2, (p.front[3] - lip + bottom) / 2, "#ffc227"),
            w: p.front[2] - p.front[0] + 48,
            h: bottom - (p.front[3] - lip),
            radius: 0,
            paint: linear(90, "#ffd152", "#ffc227", "#ffb403"),
            name: t("composer.templateLayers.bottomEdge"),
          },
        ],
        "folder",
        "windows",
      );
    },
  },
  {
    id: "colour",
    make: () => doc([makeFill(solid("#ff7a59"))]),
  },
  {
    id: "label",
    make: (p) => {
      const c = centreOf(p.front);
      return doc([makeFill(linear(180, "#2b4c7e", "#1c3257")), { ...makeText(t("composer.templateWords.projects"), c.x, c.y, "#ffffff"), size: 150 }]);
    },
  },
  {
    id: "emoji",
    make: (p) => {
      const c = centreOf(p.front);
      return doc([makeFill(linear(160, "#ffd3a5", "#fd6585")), { ...makeEmoji("📸", c.x, c.y + 6), size: 400 }]);
    },
  },
  {
    id: "tab",
    make: (p) => {
      const c = centreOf(p.front);
      const tab = centreOf(p.tab);
      return doc([
        makeFill(linear(150, "#8f5cff", "#5b2bd9")),
        { ...makeText(t("composer.templateWords.ideas"), tab.x, tab.y + 3, "#ffffff"), size: 44, weight: 900, spacing: 0.08, font: "system" },
        { ...makeEmoji("💡", c.x, c.y + 6), size: 380 },
      ]);
    },
  },
  {
    id: "two-tone",
    make: (p) => {
      // The split sits behind the paper sheet, so the back is one colour and the front another.
      // Windows' folder has no sheet (its `paper` is the back's body, as wide as the back), so
      // there the split is the front's top edge: right of the tab exactly, and under the tab,
      // where the front's edge steps lower, a strip of the back takes the front's colour.
      const sheet = p.paper[0] > p.back[0];
      const split = sheet ? (p.paper[1] + p.front[1]) / 2 : p.front[1];
      const tab = centreOf(p.tab);
      return doc([
        makeFill(solid("#ffb703")),
        { ...makeShape("rect", 512, (split + 1100) / 2, "#023047"), w: 1100, h: 1100 - split, radius: 0, name: t("composer.templateLayers.front") },
        { ...makeText(t("composer.templateWords.work"), tab.x, tab.y + 3, "#023047"), size: 46, weight: 900, spacing: 0.1, font: "system" },
      ]);
    },
  },
  {
    id: "glass",
    make: (p) => {
      const c = centreOf(p.front);
      return doc([
        makeFill(solid("#ffffff2e")),
        { ...makeFill(linear(180, "#ffffff66", "#ffffff00", "#ffffff00")), name: t("composer.templateLayers.shine") },
        { ...makeEmoji("💎", c.x, c.y + 6), size: 340 },
      ]);
    },
  },
  {
    id: "tinted",
    make: (p) => {
      const c = centreOf(p.front);
      return doc([makeFill(solid("#3a86ff70")), { ...makeText(t("composer.templateWords.drafts"), c.x, c.y, "#ffffff"), size: 140, opacity: 0.9 }]);
    },
  },
  {
    id: "stripes",
    make: (p) => {
      const c = centreOf(p.front);
      return doc([
        makeFill(solid("#ff5a5f")),
        { ...makePattern("stripes", "#ffffff4d"), scale: 76, angle: 45 },
        { ...makeShape("rect", c.x, c.y, "#ffffff"), w: 560, h: 210, radius: 1, name: t("composer.templateLayers.label") },
        { ...makeText(t("composer.templateWords.notes"), c.x, c.y, "#ff5a5f"), size: 120 },
      ]);
    },
  },
  {
    id: "gingham",
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
    make: () => doc([makeFill(solid("#1d3557")), { ...makePattern("dots", "#f1faeecc"), scale: 96 }]),
  },
  {
    id: "sunset",
    make: () =>
      doc([makeFill(linear(180, "#fbd07c", "#f7797d", "#6e48aa")), { ...makePattern("grain", "#ffffff"), scale: 2, opacity: 0.35, name: t("composer.templateLayers.grain") }]),
  },
  {
    id: "neon",
    make: (p) => {
      const c = centreOf(p.front);
      return doc([
        makeFill(linear(180, "#15102b", "#0a0818")),
        {
          ...makeText(t("composer.templateWords.night"), c.x, c.y, "#ffd6f5"),
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
    make: (p) => {
      const c = centreOf(p.front);
      return doc([
        makeFill(solid("#2a9d8f")),
        { ...makeShape("burst", c.x, c.y, "#e9c46a"), w: 470, h: 470, rotation: -8 },
        { ...makeText(t("composer.templateWords.new"), c.x, c.y + 4, "#264653"), size: 132, font: "poster", weight: 400, rotation: -8 },
      ]);
    },
  },
  {
    id: "photo",
    photo: true,
    make: (p, pic) => {
      const layers: Layer[] = [makeFill(solid("#20242c"))];
      if (pic) layers.push(makeImage(pic.src, pic.width, pic.height, imageBox(pic.width, pic.height, p, !pic.alpha)));
      const band = 150;
      const y = p.front[3] - band / 2 - 30;
      layers.push({ ...makeShape("rect", 512, y, "#00000066"), w: 1100, h: band, radius: 0, name: t("composer.templateLayers.captionBand") });
      layers.push({ ...makeText(t("composer.templateWords.summer"), 512, y, "#ffffff"), size: 76, weight: 800 });
      return doc(layers);
    },
  },
  {
    id: "sticker",
    make: () =>
      doc(
        [{ ...makeEmoji("🐱", 512, 530), size: 720, edge: { color: "#ffffff", width: 30 }, shadow: { color: "#00000047", blur: 34, x: 0, y: 18 } }],
        "free",
      ),
  },
];

export const templateById = (id: string) => TEMPLATES.find((t) => t.id === id);
