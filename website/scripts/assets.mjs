import { copyFileSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";

export function prepareAssets() {
  const files = [
    ["public/brand-mark.png", "brand-mark.png"],
    ["public/fonts/Manrope-variable.ttf", "fonts/Manrope-variable.ttf"],
    ["public/fonts/OFL.txt", "fonts/OFL.txt"],
    ["docs/images/app.webp", "media/app.webp"],
    ["docs/images/app-dark.png", "media/app-dark.png"],
    ["docs/images/app.webp", "social-preview.webp"],
    ...[
      "mona-lisa",
      "baby-blue-ice",
      "ada-lovelace",
      "the-ninth-wave",
      "statue-disco",
    ].map((name) => [
      `src/assets/onboarding/${name}.webp`,
      `skins/${name}.webp`,
    ]),
  ];
  for (const [source, destination] of files) {
    const target = new URL(`../public/${destination}`, import.meta.url);
    mkdirSync(new URL(".", target), { recursive: true });
    copyFileSync(new URL(`../../${source}`, import.meta.url), target);
  }
  return fileURLToPath(new URL("../public", import.meta.url));
}
