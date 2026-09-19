/**
 * The pictures the first-launch intro flips through: paintings from Classic Art and portraits from
 * Scientists - Pop Art, with one plain folder each from Soft Rainbow and Colours, rendered by the
 * app's own compositor at 512 px (the same pixels the gallery shows), and the FolderSkin logo the
 * intro lands on.
 *
 * They're the only pictures the app ships, so they stay small (WebP). The order is the intro's:
 * with `frameAt` in src/lib/onboarding.ts, every wave alternates packs, so no two folders side by
 * side come from the same one, the middle folder always wears a painting or a portrait, and a
 * plain folder turns up at most once a wave, at an end (src/lib/onboarding.test.ts holds all of
 * it).
 *
 * To make one again, from a pack picture (or assets/logo/folderskin-logo-cutout.png for the logo,
 * at -q 88 -alpha_q 100):
 *
 *   cargo run -p folderskin-tools -- render community/packs/classic-art/mona-lisa.webp --size 512 --out mona-lisa.png
 *   cwebp -q 78 -alpha_q 90 -m 6 -sharp_yuv mona-lisa.png -o src/assets/onboarding/mona-lisa.webp
 */
import adaLovelace from "./ada-lovelace.webp";
import archimedes from "./archimedes.webp";
import babyBlueIce from "./baby-blue-ice.webp";
import blue from "./blue.webp";
import girlWithAPearlEarring from "./girl-with-a-pearl-earring.webp";
import green from "./green.webp";
import isaacNewton from "./isaac-newton.webp";
import logo from "./logo.webp";
import marieCurie from "./marie-curie.webp";
import monaLisa from "./mona-lisa.webp";
import nikolaTesla from "./nikola-tesla.webp";
import orange from "./orange.webp";
import purple from "./purple.webp";
import theNinthWave from "./the-ninth-wave.webp";
import theStarryNight from "./the-starry-night.webp";
import wandererAboveTheSeaOfFog from "./wanderer-above-the-sea-of-fog.webp";

/** Every frame of the intro and the pack it comes from, in the intro's order. */
export const INTRO: { src: string; pack: string }[] = [
  { src: monaLisa, pack: "classic-art" },
  { src: theNinthWave, pack: "classic-art" },
  // the middle folder in the third wave
  { src: archimedes, pack: "scientists-pop-art" },
  // at the left end of the last wave
  { src: orange, pack: "colours" },
  // the middle folder in the first wave
  { src: theStarryNight, pack: "classic-art" },
  { src: wandererAboveTheSeaOfFog, pack: "classic-art" },
  { src: marieCurie, pack: "scientists-pop-art" },
  // the middle folder in the last wave
  { src: adaLovelace, pack: "scientists-pop-art" },
  // at the right end of the first wave
  { src: babyBlueIce, pack: "soft-rainbow" },
  // the middle folder in the second wave
  { src: girlWithAPearlEarring, pack: "classic-art" },
  { src: isaacNewton, pack: "scientists-pop-art" },
  { src: nikolaTesla, pack: "scientists-pop-art" },
];

export const INTRO_FRAMES = INTRO.map((frame) => frame.src);

/** Plain coloured folders, for the browser preview's stand-in skins. */
export const COLOUR_FOLDERS = [blue, orange, purple, green];

export const LOGO = logo;
