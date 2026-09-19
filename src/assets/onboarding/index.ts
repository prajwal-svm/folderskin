/**
 * The pictures the first-launch intro flips through: folders from the Classic Art and Colours
 * community packs, rendered by the app's own compositor at 512 px (the same pixels the gallery
 * shows), and the FolderSkin logo the intro lands on.
 *
 * They're the only pictures the app ships, so they stay small (WebP, about 330 KB together with
 * the logo). The order is the intro's: `frameAt` in src/lib/onboarding.ts keeps the middle folder
 * on paintings and scatters the plain colours around it.
 *
 * To make one again, from a pack picture (or assets/logo/folderskin-logo-cutout.png for the logo,
 * at -q 88 -alpha_q 100):
 *
 *   cargo run -p folderskin-tools -- render community/packs/classic-art/mona-lisa.webp --size 512 --out mona-lisa.png
 *   cwebp -q 78 -alpha_q 90 -m 6 -sharp_yuv mona-lisa.png -o src/assets/onboarding/mona-lisa.webp
 */
import blue from "./blue.webp";
import compositionViii from "./composition-viii.webp";
import girlWithAPearlEarring from "./girl-with-a-pearl-earring.webp";
import green from "./green.webp";
import logo from "./logo.webp";
import monaLisa from "./mona-lisa.webp";
import orange from "./orange.webp";
import purple from "./purple.webp";
import theLadyOfShalott from "./the-lady-of-shalott.webp";
import theNinthWave from "./the-ninth-wave.webp";
import theStarryNight from "./the-starry-night.webp";
import viewOfToledo from "./view-of-toledo.webp";
import wandererAboveTheSeaOfFog from "./wanderer-above-the-sea-of-fog.webp";

export const INTRO_FRAMES = [
  monaLisa,
  blue,
  theStarryNight,
  viewOfToledo,
  girlWithAPearlEarring,
  orange,
  purple,
  compositionViii,
  wandererAboveTheSeaOfFog,
  theNinthWave,
  green,
  theLadyOfShalott,
];

/** Plain coloured folders, for the browser preview's stand-in skins. */
export const COLOUR_FOLDERS = [blue, orange, purple, green];

export const LOGO = logo;
