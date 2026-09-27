/**
 * Every shape with its picture (`api.shapes`), asked for once and kept for the whole session:
 * the AI view, the composer and the warm-up after launch (src/lib/warmUp.ts) all read this one
 * list, so whichever asks first has it drawn and the others find it ready.
 */
import { api } from "./tauri";
import { SHAPE_PICTURE_SIZE, type ShapeInfo } from "./shapes";

let asked: Promise<ShapeInfo[]> | null = null;
let kept: ShapeInfo[] | null = null;

/** The shapes, drawn the first time anything asks and the same list every time after. A failed
 *  ask is forgotten, so the next one tries again. */
export function loadShapes(): Promise<ShapeInfo[]> {
  asked ??= api.shapes(SHAPE_PICTURE_SIZE).then(
    (list) => {
      kept = list;
      return list;
    },
    (e: unknown) => {
      asked = null;
      throw e;
    },
  );
  return asked;
}

/** The shapes if they're in already, so a view can draw them from its very first frame. */
export function shapesNow(): ShapeInfo[] | null {
  return kept;
}
