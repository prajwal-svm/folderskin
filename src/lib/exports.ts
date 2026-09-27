import type { Os } from "./platform";

/** The kinds of file a skin is saved as, and `folder`: a new folder wearing it. */
export type ExportKind = "icns" | "ico" | "png" | "jpeg" | "iconset" | "ios" | "favicon" | "folder";

/**
 * What each kind's name ends in, if anything: a file's extension, or the ending Xcode and
 * iconutil know a folder of icons by. A website's favicons and a new folder are named as typed.
 */
export const EXPORT_ENDINGS: Record<ExportKind, string | null> = {
  icns: "icns",
  ico: "ico",
  png: "png",
  jpeg: "jpg",
  iconset: "iconset",
  ios: "appiconset",
  favicon: null,
  folder: null,
};

/** Whether `kind` is one file, which a save dialog can offer by its extension. */
export const isFile = (kind: ExportKind) => kind === "icns" || kind === "ico" || kind === "png" || kind === "jpeg";

/** The order they're offered in, after this system's own kind. */
const ORDER: ExportKind[] = ["icns", "ico", "png", "jpeg", "iconset", "ios", "favicon", "folder"];

/** The kinds in the order they're offered on `os`: its own icon file first. */
export function exportKinds(os: Os): ExportKind[] {
  const own: ExportKind = os === "macos" ? "icns" : os === "windows" ? "ico" : "png";
  return [own, ...ORDER.filter((k) => k !== own)];
}

/** The name a save dialog suggests for `kind`, from `base`, a skin's name made fit for a file. */
export function suggestedName(base: string, kind: ExportKind): string {
  const ending = EXPORT_ENDINGS[kind];
  if (kind === "favicon") return `${base} favicon`;
  return ending ? `${base}.${ending}` : base;
}

/**
 * `picked`, the path a save dialog gave, with `kind`'s ending added when it was typed without one
 * (some Linux dialogs add none). A JPEG may end in .jpeg too.
 */
export function withEnding(picked: string, kind: ExportKind): string {
  const ending = EXPORT_ENDINGS[kind];
  if (!ending) return picked;
  const endings = kind === "jpeg" ? ["jpg", "jpeg"] : [ending];
  const lower = picked.toLowerCase();
  return endings.some((e) => lower.endsWith(`.${e}`)) ? picked : `${picked}.${ending}`;
}
