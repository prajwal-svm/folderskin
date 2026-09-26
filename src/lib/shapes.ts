/**
 * The shapes the AI chat paints on: a folder in each system's look, and a free icon with no base
 * at all. The list comes from the app (ai.rs `ai_shapes`, folderskin_core::base), so a shape the
 * app gains shows up here without a change; the chat names each by its id, in the language on
 * show when the catalog has it (`common.shapes.<id>`), and in the app's own English otherwise.
 */
import common from "../locales/en/common.json";
import { t, type MessageKey } from "../i18n";
import type { FolderStyle } from "../composer/parts";

/** One shape, as ai.rs `AiShapeDto` sends it. */
export type ShapeInfo = {
  id: string;
  /** Its name in English, for a shape the catalog doesn't name yet. */
  label: string;
  system: string;
  /** "folder", "drive" or "free": where its skins go in the library. */
  family: string;
  /** Whether it can be painted whole as well as as artwork. */
  whole: boolean;
  /** The shape bare, as a data URL; none for a free icon. */
  thumbnail: string | null;
};

const NAMED = new Set(Object.keys(common.shapes));

/** A shape's name in the language on show. */
export function shapeName(shape: Pick<ShapeInfo, "id" | "label">): string {
  return NAMED.has(shape.id) ? t(`common.shapes.${shape.id}` as MessageKey) : shape.label;
}

/** What a shape is, in a line, when the catalog says; empty for one it doesn't know yet. */
export function shapeNote(shape: Pick<ShapeInfo, "id">): string {
  return NAMED.has(shape.id) ? t(`common.shapeNotes.${shape.id}` as MessageKey) : "";
}

/**
 * The shape a chat is on: the one it names, or, for a new chat or one naming a shape this build
 * doesn't have, the folder the app puts skins on (the folder look chosen in the folder panel).
 */
export function shapeOf(shapes: ShapeInfo[], id: string | null | undefined, look: FolderStyle): ShapeInfo | undefined {
  return (id ? shapes.find((s) => s.id === id) : undefined) ?? defaultShape(shapes, look);
}

/** The folder in `look`, which is what a picture is made for when nobody picks. */
export function defaultShape(shapes: ShapeInfo[], look: FolderStyle): ShapeInfo | undefined {
  return shapes.find((s) => s.family === "folder" && s.system === look) ?? shapes.find((s) => s.family === "folder") ?? shapes[0];
}

/** The shapes grouped the way the picker lists them: folders, then drives, then a free icon, each in the order the app gave them. */
export function groupShapes(shapes: ShapeInfo[]): { family: string; shapes: ShapeInfo[] }[] {
  const order = ["folder", "drive", "free"];
  const families = [...new Set(shapes.map((s) => s.family))].sort((a, b) => rank(order, a) - rank(order, b));
  return families.map((family) => ({ family, shapes: shapes.filter((s) => s.family === family) }));
}

const rank = (order: string[], family: string) => (order.includes(family) ? order.indexOf(family) : order.length);

/** Letters without their accents, in lower case, for matching what's typed. */
export function fold(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").toLocaleLowerCase();
}

/**
 * How well `query` matches something called `name`, described by `more`: 3 when the name starts
 * with it, 2 when a word of the name does, 1 when every word of it is found somewhere, 0 when it
 * isn't. An empty query matches everything, as 1.
 */
export function matchScore(query: string, name: string, more = ""): number {
  const q = fold(query.trim());
  if (!q) return 1;
  const n = fold(name);
  if (n.startsWith(q)) return 3;
  const words = n.split(/[\s\-_/]+/);
  if (words.some((w) => w.startsWith(q))) return 2;
  const all = `${n} ${fold(more)}`;
  return q.split(/\s+/).every((w) => all.includes(w)) ? 1 : 0;
}

/** The shapes `query` matches, best first, keeping the picker's order among equals. */
export function filterShapes(shapes: ShapeInfo[], query: string): ShapeInfo[] {
  return shapes
    .map((shape, i) => ({ shape, i, score: matchScore(query, shapeName(shape), `${shape.id} ${shape.label} ${shapeNote(shape)}`) }))
    .filter((s) => s.score > 0)
    .sort((a, b) => b.score - a.score || a.i - b.i)
    .map((s) => s.shape);
}
