import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { readFileSync } from "node:fs";

const pkg = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8")) as { version: string };

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  define: { __APP_VERSION__: JSON.stringify(pkg.version) },
  clearScreen: false,
  server: {
    port: 14200,
    strictPort: true,
    host: host || false,
    watch: { ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"] },
  },
  build: {
    target: ["es2022", "safari15"],
    minify: true,
    sourcemap: false,
    rollupOptions: {
      output: {
        // English is in the first chunk. Every other language is one chunk of its own, all its
        // namespaces together, loaded the first time it's shown (src/i18n/index.ts).
        manualChunks(id) {
          const locale = /\/src\/locales\/([^/]+)\/[^/]+\.json$/.exec(id.replace(/\\/g, "/"))?.[1];
          return locale && locale !== "en" ? `locale-${locale}` : undefined;
        },
      },
    },
  },
  test: {
    environment: "node",
    // The check script for the translations (scripts/check-locales.mjs) is tested alongside.
    include: ["src/**/*.test.ts", "scripts/**/*.test.mjs"],
    // `pnpm test:coverage` writes the report SonarQube Cloud reads (sonar-project.properties).
    // Files no test runs are listed too, as uncovered.
    coverage: {
      provider: "v8",
      reporter: ["text-summary", "lcov"],
      reportsDirectory: "coverage/frontend",
      include: ["src/**/*.{ts,tsx}"],
      exclude: ["src/**/*.test.ts", "src/**/*.d.ts"],
    },
  },
});
