/**
 * What colour a skin is, for the gallery's colour filter: up to three colour names that each
 * cover a fair part of the folder, and whether it's light or dark overall.
 *
 * The reading itself happens in Rust (`folderskin-core::palette`), where the thumbnail is already
 * in memory, and arrives on the skin. This file is the vocabulary the filter draws with: the
 * names, and the swatch for each. Keep the names in step with the Rust enum — they are what
 * crosses between them.
 */

export type Colour =
  | "red"
  | "orange"
  | "yellow"
  | "green"
  | "teal"
  | "blue"
  | "purple"
  | "pink"
  | "brown"
  | "black"
  | "grey"
  | "white";

export type Tone = "light" | "dark";

export type Palette = { colours: Colour[]; tone: Tone };

/** The colours the filter offers, in the order it shows them, with the swatch it draws. */
export const COLOURS: { id: Colour; label: string; swatch: string }[] = [
  { id: "red", label: "Red", swatch: "#e5484d" },
  { id: "orange", label: "Orange", swatch: "#f76b15" },
  { id: "yellow", label: "Yellow", swatch: "#f5c518" },
  { id: "green", label: "Green", swatch: "#30a46c" },
  { id: "teal", label: "Teal", swatch: "#12a594" },
  { id: "blue", label: "Blue", swatch: "#3a86ff" },
  { id: "purple", label: "Purple", swatch: "#8e4ec6" },
  { id: "pink", label: "Pink", swatch: "#e93d82" },
  { id: "brown", label: "Brown", swatch: "#8d5a2b" },
  { id: "black", label: "Black", swatch: "#1d1d1f" },
  { id: "grey", label: "Grey", swatch: "#8b8d98" },
  { id: "white", label: "White", swatch: "#f4f4f5" },
];
