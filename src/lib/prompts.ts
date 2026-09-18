/**
 * Art direction shared by the assistant and the "use your own chat" helper. The same text is
 * in docs/PROMPTS.md; keep them in step.
 */

/** Styles that hold up at folder size: strong shapes, clear light, a recognisable technique. */
export const STYLES: { id: string; label: string; text: string }[] = [
  { id: "travel", label: "Travel poster", text: "vintage travel poster, flat colour, grainy print texture" },
  { id: "ukiyoe", label: "Woodblock", text: "ukiyo-e woodblock print with bold outlines" },
  { id: "airbrush", label: "70s airbrush", text: "1970s airbrushed poster with glossy chrome" },
  { id: "collage", label: "Collage", text: "surreal photo collage with cut-paper edges and halftone dots" },
  { id: "nouveau", label: "Art nouveau", text: "art nouveau poster with ornate borders and thin gold lines" },
  { id: "oil", label: "Oil painting", text: "Renaissance oil painting with dramatic light" },
  { id: "film", label: "Film still", text: "cinematic photograph at golden hour, 35 mm film grain" },
  { id: "diorama", label: "Tiny diorama", text: "miniature diorama shot with a tilt-shift lens" },
  { id: "riso", label: "Risograph", text: "risograph print in three inks" },
  { id: "clay", label: "Clay", text: "soft clay render, like a stop-motion set" },
];

/** Scenes to start from when the box is empty. */
export const SCENES = [
  "an astronaut watering a tiny garden on the moon",
  "a koi fish swimming through a rainy neon street",
  "a red phone booth standing alone on a glacier",
  "a whale drifting over a sleepy seaside village at dawn",
  "a cat in sunglasses running a tiny coffee cart",
  "a vinyl record rising like the sun over a desert",
  "a lighthouse throwing a rainbow into a thunderstorm",
  "a retro computer on a beach, its screen showing a sunset",
  "a tiger surfing an enormous wave",
  "a tiny office inside a teacup, people typing at their desks",
];

/** The idea plus its style, the way the assistant sends it. */
export function withStyle(idea: string, styleId: string | null): string {
  const style = STYLES.find((s) => s.id === styleId);
  return style ? `${idea.trim()}. Style: ${style.text}` : idea.trim();
}

/** The full prompt for Grok or ChatGPT's own chat, used with docs/prompts/folder-template.png. */
export function chatPrompt(scene: string, styleId: string | null): string {
  const style = STYLES.find((s) => s.id === styleId)?.text ?? "DESCRIBE THE STYLE";
  return `Repaint the attached folder icon. Keep its exact shape: the same folder silhouette, the tab on the top left, the thin cream paper sheet between the back and front panels, and the same size and position in the frame. Straight-on front view, no perspective, no tilt.

Scene: ${scene.trim() || "DESCRIBE THE SCENE"}
Style: ${style}

Paint the scene across the whole folder. The sky or background continues up into the back panel and the tab; the main subject sits in the middle of the front panel, fully inside it. Keep the paper sheet as a clean cream strip. Rich colour, strong light, fine texture, like a collectible poster.

Everything outside the folder stays pure flat magenta #FF00FF: no shadow, no glow, no gradient, no border, no other objects. Do not use magenta or hot pink inside the folder. Square image.`;
}
