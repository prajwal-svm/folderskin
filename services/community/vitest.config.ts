import { cloudflareTest, readD1Migrations } from "@cloudflare/vitest-pool-workers";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

// The tests run inside workerd with local D1, R2 and rate-limit simulators, and never reach the
// network: Workers AI is left out of the bindings they use (test/helpers.ts), and anything else the
// service fetches (Turnstile, webhooks) is stubbed in the test that expects it. `outboundService`
// answers every other request with an error, so a test that forgot a stub fails instead of going online.
export default defineConfig(async () => {
  const migrations = await readD1Migrations(fileURLToPath(new URL("./migrations", import.meta.url)));
  return {
    plugins: [
      cloudflareTest({
        main: "./src/index.ts",
        remoteBindings: false,
        wrangler: { configPath: "./wrangler.toml" },
        miniflare: {
          bindings: { TEST_MIGRATIONS: migrations },
          outboundService: () => new Response("The tests don't go online.", { status: 599 }),
        },
      }),
    ],
    test: {
      setupFiles: ["./test/setup.ts"],
      // obscenity ships CommonJS behind an ESM wrapper. wrangler's bundler joins the two when it
      // deploys; the test runner loads modules one by one, so it gets them pre-bundled instead.
      deps: { optimizer: { ssr: { enabled: true, include: ["obscenity"] } } },
    },
  };
});
