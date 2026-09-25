/**
 * Paints as the composer's controls show them: ready-made gradients, and any paint as a CSS
 * background for a swatch or a layer's thumbnail.
 */
import { cssColor } from "./color";
import { linear, radial, type Paint } from "./doc";

/** Gradients one click away. */
export const GRADIENTS: { id: string; paint: Paint }[] = [
  { id: "sky", paint: linear(180, "#7cc8f5", "#4ea9e4") },
  { id: "dusk", paint: linear(90, "#696eff", "#f8acff") },
  { id: "peach", paint: linear(160, "#ffd3a5", "#fd6585") },
  { id: "mint", paint: linear(135, "#d4fc79", "#96e6a1") },
  { id: "ocean", paint: linear(160, "#2af598", "#009efd") },
  { id: "grape", paint: linear(135, "#c471f5", "#fa71cd") },
  { id: "night", paint: linear(180, "#243b55", "#141e30") },
  { id: "candy", paint: linear(120, "#ff9a9e", "#fecfef") },
  { id: "ember", paint: linear(90, "#f74c06", "#f9bc2c") },
  { id: "lagoon", paint: radial(0.5, 0.35, "#a1ffce", "#1c92d2") },
  { id: "glow", paint: radial(0.5, 0.5, "#ffffff", "#3a86ff") },
  { id: "mono", paint: linear(180, "#f5f5f5", "#9e9e9e") },
];

/** A CSS picture of a paint, for swatches. */
export function paintCss(paint: Paint): string {
  if (paint.type === "solid") return cssColor(paint.color);
  const stops = paint.stops.map((s) => `${cssColor(s.color)} ${Math.round(s.at * 100)}%`).join(", ");
  return paint.type === "linear"
    ? `linear-gradient(${paint.angle}deg, ${stops})`
    : `radial-gradient(circle at ${paint.cx * 100}% ${paint.cy * 100}%, ${stops})`;
}
