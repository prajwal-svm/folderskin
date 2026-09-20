import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";
import { prepareAssets } from "./scripts/assets.mjs";

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  base: process.env.SITE_BASE || "/folderskin/",
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
