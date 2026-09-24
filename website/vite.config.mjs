import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";
import { prepareAssets } from "./scripts/assets.mjs";

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  // Relative asset addresses: the same build works at folderskin.app/ and at
  // prajwal-svm.github.io/folderskin/, which redirects there. SITE_BASE sets another.
  base: process.env.SITE_BASE || "./",
  publicDir: prepareAssets(),
  server: {
    host: "127.0.0.1",
    port: 4173,
    strictPort: true,
    fs: { allow: [".."] },
  },
  preview: { host: "127.0.0.1", port: 4174, strictPort: true },
  build: { outDir: "dist", target: ["es2022", "safari15"], sourcemap: false },
});
