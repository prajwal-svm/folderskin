/**
 * The pictures the first-launch intro shows, and the FolderSkin logo it lands on. The folders come
 * from the community packs, Classic Art paintings and Scientists - Pop Art portraits with one plain
 * folder each from Soft Rainbow and Colours, and three marble statues from a pack on its way.
 * Each shows once: five for every wave, then one for every flip of the middle folder
 * (INTRO_FRAME_COUNT in src/lib/onboarding.ts). Side by side, folders never come from the same
 * pack, the middle one always wears a picture, and a plain folder only turns up at an end
 * (src/lib/onboarding.test.ts holds all of it).
 *
 * They're the only pictures the app ships, so each is drawn at the size it's shown at: 512 px for
 * the middle folder, 384 px beside it, 288 px at the ends, and 320 px for the quick flips, whose
 * frames are gone in 75 ms. The app's own compositor draws them (the gallery's pixels):
 *
 *   cargo run -p folderskin-tools -- render community/packs/classic-art/mona-lisa.webp --size 384 --out mona-lisa.png
 *   cwebp -q 78 -alpha_q 90 -m 6 -sharp_yuv mona-lisa.png -o src/assets/onboarding/mona-lisa.webp
 *
 * (-q 70 for the flips; the logo is assets/logo/folderskin-logo-cutout.png at -q 88 -alpha_q 100.)
 */
import adaLovelace from "./ada-lovelace.webp";
import alanTuring from "./alan-turing.webp";
import archimedes from "./archimedes.webp";
import babyBlueIce from "./baby-blue-ice.webp";
import blue from "./blue.webp";
import breezingUp from "./breezing-up.webp";
import carlSagan from "./carl-sagan.webp";
import charlesDarwin from "./charles-darwin.webp";
import compositionViii from "./composition-viii.webp";
import galileoGalilei from "./galileo-galilei.webp";
import girlWithAPearlEarring from "./girl-with-a-pearl-earring.webp";
import graceHopper from "./grace-hopper.webp";
import green from "./green.webp";
import isaacNewton from "./isaac-newton.webp";
import katherineJohnson from "./katherine-johnson.webp";
import logo from "./logo.webp";
import luncheonOfTheBoatingParty from "./luncheon-of-the-boating-party.webp";
import marieCurie from "./marie-curie.webp";
import monaLisa from "./mona-lisa.webp";
import napoleonCrossingTheAlps from "./napoleon-crossing-the-alps.webp";
import nikolaTesla from "./nikola-tesla.webp";
import orange from "./orange.webp";
import purple from "./purple.webp";
import statueDisco from "./statue-disco.webp";
import statueGaze from "./statue-gaze.webp";
import statueOpenArms from "./statue-open-arms.webp";
import theAstronomer from "./the-astronomer.webp";
import theLadyOfShalott from "./the-lady-of-shalott.webp";
import theNinthWave from "./the-ninth-wave.webp";
import wandererAboveTheSeaOfFog from "./wanderer-above-the-sea-of-fog.webp";

const CLASSIC = "classic-art";
const SCIENTISTS = "scientists-pop-art";
/** The marble statues' pack, not published yet. */
const STATUES = "statues";

/** Every frame of the intro and the pack it comes from, in the order they show. */
export const INTRO: { src: string; pack: string }[] = [
  // the first wave, left to right; the middle one is the biggest
  { src: galileoGalilei, pack: SCIENTISTS },
  { src: monaLisa, pack: CLASSIC },
  { src: statueDisco, pack: STATUES },
  { src: marieCurie, pack: SCIENTISTS },
  { src: babyBlueIce, pack: "soft-rainbow" },
  // the second wave
  { src: theNinthWave, pack: CLASSIC },
  { src: archimedes, pack: SCIENTISTS },
  { src: girlWithAPearlEarring, pack: CLASSIC },
  { src: nikolaTesla, pack: SCIENTISTS },
  { src: orange, pack: "colours" },
  // the third wave
  { src: wandererAboveTheSeaOfFog, pack: CLASSIC },
  { src: adaLovelace, pack: SCIENTISTS },
  { src: statueGaze, pack: STATUES },
  { src: theLadyOfShalott, pack: CLASSIC },
  { src: isaacNewton, pack: SCIENTISTS },
  // the last wave
  { src: charlesDarwin, pack: SCIENTISTS },
  { src: napoleonCrossingTheAlps, pack: CLASSIC },
  { src: statueOpenArms, pack: STATUES },
  { src: luncheonOfTheBoatingParty, pack: CLASSIC },
  { src: graceHopper, pack: SCIENTISTS },
  // the middle folder's flips on its way to the logo
  { src: theAstronomer, pack: CLASSIC },
  { src: alanTuring, pack: SCIENTISTS },
  { src: breezingUp, pack: CLASSIC },
  { src: katherineJohnson, pack: SCIENTISTS },
  { src: compositionViii, pack: CLASSIC },
  { src: carlSagan, pack: SCIENTISTS },
];

export const INTRO_FRAMES = INTRO.map((frame) => frame.src);

/** Plain coloured folders, for the browser preview's stand-in skins. */
export const COLOUR_FOLDERS = [blue, orange, purple, green];

export const LOGO = logo;
