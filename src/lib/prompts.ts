/**
 * Art direction shared by the assistant and the "use your own chat" helper. The same text is
 * in docs/PROMPTS.md; keep them in step.
 */

/** The styles' ids; each chip is named by `ai.styles.<id>`. */
export type StyleId = "travel" | "ukiyoe" | "airbrush" | "collage" | "nouveau" | "oil" | "film" | "diorama" | "riso" | "clay";

/**
 * Styles that hold up at folder size: strong shapes, clear light, a recognisable technique.
 * `tag` is what an AI result in that style is tagged with, and `words` is how a description
 * gives the style away. `text`, the briefs below and the prompt for the chat apps stay in
 * English whatever language the app is in: they're what the models read, and they read English
 * best.
 */
export const STYLES: { id: StyleId; text: string; tag: string; words: RegExp }[] = [
  { id: "travel", text: "vintage travel poster, flat colour, grainy print texture", tag: "travel poster", words: /travel poster/i },
  { id: "ukiyoe", text: "ukiyo-e woodblock print with bold outlines", tag: "woodblock", words: /ukiyo|woodblock/i },
  { id: "airbrush", text: "1970s airbrushed poster with glossy chrome", tag: "airbrush", words: /airbrush/i },
  { id: "collage", text: "surreal photo collage with cut-paper edges and halftone dots", tag: "collage", words: /collage/i },
  { id: "nouveau", text: "art nouveau poster with ornate borders and thin gold lines", tag: "art nouveau", words: /art nouveau/i },
  { id: "oil", text: "Renaissance oil painting with dramatic light", tag: "oil painting", words: /oil painting/i },
  { id: "film", text: "cinematic photograph at golden hour, 35 mm film grain", tag: "film still", words: /film still|35 ?mm|cinematic/i },
  { id: "diorama", text: "miniature diorama shot with a tilt-shift lens", tag: "diorama", words: /diorama|tilt-shift|miniature/i },
  { id: "riso", text: "risograph print in three inks", tag: "risograph", words: /risograph/i },
  { id: "clay", text: "soft clay render, like a stop-motion set", tag: "clay", words: /\bclay|claymation|stop-motion/i },
];

/** Tags for an AI result: the styles its description asks for, such as "airbrush". */
export function styleTags(idea: string): string[] {
  return STYLES.filter((s) => s.words.test(idea)).map((s) => s.tag);
}

/**
 * Ready-to-send briefs, two per style. Each is one concept with a twist, one clear subject for
 * the front panel, the style named, and the texture or light that makes it read like a poster:
 * the qualities that made the best folder art look collectible. No magenta or hot pink, which
 * the cut-out would eat.
 */
export const SUGGESTIONS: Record<string, string[]> = {
  travel: [
    "A tiny red seaplane landing on a turquoise lagoon at sunset, palm silhouettes and a low orange sun, as a vintage travel poster in flat colours with grainy print texture and the word ESCAPE in bold retro letters.",
    "A cable car climbing past snowy peaks toward a little alpine hotel, as a 1950s travel poster: flat blues and whites, one red accent, soft print grain.",
  ],
  ukiyoe: [
    "A giant koi leaping over a great wave under a pale moon, as a ukiyo-e woodblock print with bold black outlines, indigo and vermilion on washi paper.",
    "A fox in a straw hat crossing a lantern-lit bridge in the rain, as an Edo-period woodblock print with flat colour blocks and fine rain lines.",
  ],
  airbrush: [
    "A chrome cassette tape floating over a neon desert highway at dusk, as a 1970s airbrushed poster with glossy highlights and a purple-to-tangerine sky.",
    "A shiny roller skate orbiting a ringed planet, as a 70s airbrush illustration with chrome reflections, lens flares and a deep violet starfield.",
  ],
  collage: [
    "A vintage astronaut floating between cut-paper clouds, holding a steaming coffee cup, as a surreal photo collage with halftone dots and torn-paper edges.",
    "A giant hand watering a tiny city skyline like a houseplant, as a retro magazine collage with halftone print, paper grain and a mustard-yellow sky.",
  ],
  nouveau: [
    "A woman whose hair turns into ocean waves, framed by lilies and ornate gold lines, as an art nouveau poster in muted teal, cream and coral.",
    "A peacock perched on a crescent moon among swirling vines, as an art nouveau poster with thin gold outlines and jewel greens and blues.",
  ],
  oil: [
    "A cat in a velvet cloak holding a tiny laptop, as a Renaissance oil painting with dramatic candlelight, deep reds and golds, and cracked varnish.",
    "A whale drifting over a sleepy harbour at dawn, as a romantic oil painting with soft clouds, visible brushwork and warm morning light.",
  ],
  film: [
    "A lone red phone booth on a snowy mountain ridge at golden hour, as a cinematic 35 mm film still with soft grain and long shadows.",
    "A vintage convertible parked under a neon diner sign on a rainy night, as a moody film still with wet reflections and teal-orange colour.",
  ],
  diorama: [
    "A busy little post office built inside a wooden drawer, tiny workers sorting letters under warm lamps, as a tilt-shift miniature diorama.",
    "A tiny campsite on top of a giant open book, with a tent, a campfire and paper pine trees, as a tilt-shift miniature photo with soft evening light.",
  ],
  riso: [
    "A vinyl record rising like the sun over desert dunes, as a three-colour risograph print in teal, yellow and orange, with visible grain and slight misregistration.",
    "A paper boat sailing through a city of stacked books, as a two-colour risograph in blue and orange with a grainy, slightly offset print.",
  ],
  clay: [
    "A tiny lighthouse on a rocky island throwing a rainbow beam through puffy clouds, as a soft clay stop-motion scene in pastel colours with fingerprints in the clay.",
    "A snail carrying a little house with glowing windows through a mossy forest, as a cosy claymation set in soft light.",
  ],
};

/** The brief a style chip puts in the box on its `index`-th click. */
export function suggestion(styleId: string, index: number): string {
  const list = SUGGESTIONS[styleId] ?? [];
  return list.length ? list[((index % list.length) + list.length) % list.length] : "";
}

/** Any brief at random, with the style it belongs to. */
export function surprise(random: () => number = Math.random): { styleId: string; index: number; text: string } {
  const style = STYLES[Math.floor(random() * STYLES.length)];
  const index = Math.floor(random() * (SUGGESTIONS[style.id]?.length ?? 1));
  return { styleId: style.id, index, text: suggestion(style.id, index) };
}

/**
 * The full prompt for Grok's or ChatGPT's own chat, used with docs/prompts/folder-template.png.
 * A description from the box usually names its style already, so the Style line is only added
 * when a style is picked in the helper (or both are still blank).
 */
export function chatPrompt(scene: string, styleId: string | null): string {
  const described = scene.trim();
  const style = STYLES.find((s) => s.id === styleId)?.text;
  const lines = [`Scene: ${described || "DESCRIBE THE SCENE"}`];
  if (style) lines.push(`Style: ${style}`);
  else if (!described) lines.push("Style: DESCRIBE THE STYLE");
  return `Repaint the attached folder icon. Keep its exact shape: the same folder silhouette, the tab on the top left, the thin cream paper sheet between the back and front panels, and the same size and position in the frame. Straight-on front view, no perspective, no tilt.

${lines.join("\n")}

Paint the scene across the whole folder. The sky or background continues up into the back panel and the tab. The main subject sits in the middle of the front panel, fully inside it. Keep the paper sheet as a clean cream strip. Rich colour, strong light, fine texture, like a collectible poster.

Paint everything outside the folder pure flat magenta #FF00FF: no shadow, no glow, no gradient, no border, no other objects. Do not use magenta or hot pink inside the folder. Square image.`;
}
