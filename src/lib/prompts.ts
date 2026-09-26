/**
 * Art direction shared by the assistant and the "use your own chat" helper: the ideas the style
 * chips fill in, and the prompt for a chat app's own chat. The styles themselves are the one
 * table in styles.ts. docs/PROMPTS.md has the same text, and prompts.test.ts keeps it in step.
 */
import { styleById, type Style } from "./styles";
import ai from "../locales/en/ai.json";
import { t, type MessageKey } from "../i18n";

export { STYLES, styleTags } from "./styles";

/**
 * Ideas to start from, two for each style a chip offers, in the order the chips show them. Each
 * is what the picture shows, never how it's rendered: the chip puts the style in its own slot,
 * so the idea stays word for word and the style is added after it. One concept with a twist,
 * one clear subject for the front panel, and the light or palette that makes it read. No magenta
 * or hot pink, which the cut-out would eat.
 */
export const SUGGESTIONS: Record<string, string[]> = {
  screenprint: [
    "A tiny red seaplane landing on a turquoise lagoon at sunset, palm silhouettes and a low orange sun, with the word “ESCAPE”.",
    "A cable car climbing past snowy peaks toward a little alpine hotel, in flat blues and whites with one red accent.",
  ],
  woodblock: [
    "A giant koi leaping from a moonlit river under a pale moon, in indigo and vermilion.",
    "A fox in a straw hat crossing a lantern-lit bridge in the rain, with fine rain lines.",
  ],
  airbrush: [
    "A chrome cassette tape floating over a desert highway at dusk, under a purple-to-tangerine sky.",
    "A shiny roller skate orbiting a ringed planet, with chrome reflections and lens flares against a deep violet starfield.",
  ],
  collage: [
    "A vintage astronaut floating between paper clouds, holding a steaming coffee cup.",
    "A giant hand watering a tiny city skyline like a houseplant, under a mustard-yellow sky.",
  ],
  "art-nouveau": [
    "A woman whose hair turns into ocean waves, among lilies, in muted teal, cream and coral.",
    "A peacock perched on a crescent moon among swirling vines, in jewel greens and blues.",
  ],
  oil: [
    "A cat in a velvet cloak holding a tiny laptop, lit by candlelight, in deep reds and golds.",
    "A whale drifting over a sleepy harbour at dawn, with soft clouds and warm morning light.",
  ],
  film: [
    "A lone red phone booth on a snowy mountain ridge at golden hour, with long shadows.",
    "A vintage convertible parked outside a glowing roadside diner on a rainy night, with wet reflections.",
  ],
  miniature: [
    "A busy little post office built inside a wooden drawer, tiny workers sorting letters under warm lamps.",
    "A tiny campsite on top of a giant open book, with a tent, a campfire and paper pine trees in soft evening light.",
  ],
  risograph: [
    "A vinyl record rising like the sun over desert dunes, in teal, yellow and orange.",
    "A paper boat sailing through a city of stacked books, in blue and orange.",
  ],
  clay: [
    "A tiny lighthouse on a rocky island throwing a rainbow beam through puffy clouds, in pastel colours.",
    "A snail carrying a little house with glowing windows through a mossy forest, in soft light.",
  ],
};

/** The idea a style chip puts in the box on its `index`-th click. */
export function suggestion(styleId: string, index: number): string {
  const list = SUGGESTIONS[styleById(styleId)?.id ?? styleId] ?? [];
  return list.length ? list[((index % list.length) + list.length) % list.length] : "";
}

/** The styles the chips under the box offer, in the order they show them. */
export const CHIP_STYLES: Style[] = Object.keys(SUGGESTIONS)
  .map((id) => styleById(id))
  .filter((s): s is Style => s !== undefined);

/** Any idea at random, with the style it goes with. */
export function surprise(random: () => number = Math.random): { styleId: string; index: number; text: string } {
  const style = CHIP_STYLES[Math.floor(random() * CHIP_STYLES.length)];
  const index = Math.floor(random() * (SUGGESTIONS[style.id]?.length ?? 1));
  return { styleId: style.id, index, text: suggestion(style.id, index) };
}

/**
 * The full prompt for Grok's or ChatGPT's own chat, used with docs/prompts/folder-template.png.
 * A description from the box usually names its style already, so the Style line is only added
 * when a style is picked (or both are still blank).
 */
export function chatPrompt(scene: string, styleId: string | null): string {
  const described = scene.trim();
  const style = styleById(styleId)?.fragment;
  const lines = [`Scene: ${described || "DESCRIBE THE SCENE"}`];
  if (style) lines.push(`Style: ${style}`);
  else if (!described) lines.push("Style: DESCRIBE THE STYLE");
  return `Repaint the attached folder icon. Keep its exact shape: the same folder silhouette, the tab on the top left, the thin cream paper sheet between the back and front panels, and the same size and position in the frame. Straight-on front view, no perspective, no tilt.

${lines.join("\n")}

Paint the scene across the whole folder. The sky or background continues up into the back panel and the tab. The main subject sits in the middle of the front panel, fully inside it. Keep the paper sheet as a clean cream strip. Rich colour, strong light, fine texture, like a collectible poster.

Paint everything outside the folder pure flat magenta #FF00FF: no shadow, no glow, no gradient, no border, no other objects. Do not use magenta or hot pink inside the folder. Square image.`;
}

/** An idea to start from (folderskin_ai's `Preset`), named in the language on show, or by the app's own English for one the catalog doesn't name yet. */
export function ideaName(preset: { id: string; label: string }): string {
  return Object.hasOwn(ai.presets, preset.id) ? t(`ai.presets.${preset.id}` as MessageKey) : preset.label;
}
