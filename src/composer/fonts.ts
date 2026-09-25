/**
 * The typefaces text layers can use. FolderSkin ships only Manrope and has no network, so the
 * rest are styles: each is a list of fonts that come with macOS, Windows or Linux, best first,
 * so every computer draws the style with the closest one it has. A design is saved as pixels,
 * so the font only needs to be on the computer it's made on.
 */
import "../i18n/composer";
import { t, type MessageKey } from "../i18n";

export type FontStyle = { id: string; stack: string };

export const FONTS: FontStyle[] = [
  { id: "rounded", stack: 'ui-rounded, "SF Pro Rounded", "Arial Rounded MT Bold", "Nunito", "Varela Round", system-ui, sans-serif' },
  { id: "manrope", stack: '"Manrope", system-ui, sans-serif' },
  { id: "system", stack: 'system-ui, -apple-system, "Segoe UI", "Helvetica Neue", Roboto, Ubuntu, sans-serif' },
  { id: "geometric", stack: 'Futura, "Century Gothic", "Avenir Next", "Trebuchet MS", "URW Gothic", sans-serif' },
  { id: "grotesk", stack: '"Helvetica Neue", Helvetica, Arial, "Liberation Sans", sans-serif' },
  { id: "condensed", stack: '"Avenir Next Condensed", "Arial Narrow", "Roboto Condensed", "Liberation Sans Narrow", sans-serif' },
  { id: "poster", stack: 'Impact, Haettenschweiler, "Arial Black", "Franklin Gothic Heavy", sans-serif' },
  { id: "serif", stack: '"New York", ui-serif, "Iowan Old Style", Georgia, Cambria, "Times New Roman", serif' },
  { id: "didone", stack: 'Didot, "Bodoni 72", "Bodoni MT", "Libre Bodoni", "Playfair Display", Georgia, serif' },
  { id: "slab", stack: 'Rockwell, "Roboto Slab", "Courier New", serif' },
  { id: "engraved", stack: 'Copperplate, "Copperplate Gothic Light", "Perpetua Titling MT", "Trajan Pro", serif' },
  { id: "mono", stack: 'ui-monospace, "SF Mono", Menlo, Consolas, "Cascadia Mono", "Liberation Mono", monospace' },
  { id: "typewriter", stack: '"American Typewriter", "Courier New", Courier, "Nimbus Mono PS", monospace' },
  { id: "hand", stack: '"Bradley Hand", "Segoe Print", "Comic Sans MS", "Comic Neue", cursive' },
  { id: "script", stack: '"Snell Roundhand", "Segoe Script", "Brush Script MT", "Apple Chancery", cursive' },
  { id: "marker", stack: '"Marker Felt", "Chalkboard SE", "Comic Sans MS", fantasy' },
  { id: "chalk", stack: 'Chalkduster, "Chalkboard SE", "Segoe Print", fantasy' },
];

/** The emoji fonts of the three systems, for emoji layers. */
export const EMOJI_STACK = '"Apple Color Emoji", "Segoe UI Emoji", "Noto Color Emoji", "Twemoji Mozilla", sans-serif';

/** A family the user typed in "Other font", kept as `custom:<family>`. */
export const CUSTOM = "custom:";

const quoted = (family: string) => `"${family.replace(/["\\]/g, "")}"`;

/** The font list to draw a font id with. An unknown id falls back to Rounded. */
export function fontStack(id: string): string {
  if (id.startsWith(CUSTOM)) {
    const family = id.slice(CUSTOM.length).trim();
    return family ? `${quoted(family)}, system-ui, sans-serif` : FONTS[0].stack;
  }
  return (FONTS.find((f) => f.id === id) ?? FONTS[0]).stack;
}

/** A font's name as the lists show it: `composer.fonts.<id>`, or a font of the computer's own by its name. */
export function fontLabel(id: string): string {
  if (id.startsWith(CUSTOM)) return id.slice(CUSTOM.length).trim() || t("composer.otherFont");
  const font = FONTS.find((f) => f.id === id) ?? FONTS[0];
  return t(`composer.fonts.${font.id}` as MessageKey);
}

export const WEIGHTS = [
  { value: 300 },
  { value: 400 },
  { value: 600 },
  { value: 800 },
  { value: 900 },
];
