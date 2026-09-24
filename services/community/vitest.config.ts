import { cloudflareTest, readD1Migrations } from "@cloudflare/vitest-pool-workers";
import { defineConfig } from "vitest/config";

// The tests run inside workerd with local D1, R2 and rate-limit simulators, and never reach the
// network: Workers AI is left out of the bindings they use (test/helpers.ts), and anything else the
// service fetches (Turnstile, webhooks) is stubbed in the test that expects it. `outboundService`
// answers every other request with an error, so a test that forgot a stub fails instead of going online.
export default defineConfig(async () => {
  // Relative to this folder, which `pnpm test` runs in, here and in CI. The service is typechecked
  // against the Workers types only, so this file can't use Node's own modules.
  const migrations = await readD1Migrations("./migrations");
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
      // A file's first test also starts its workerd and builds the profanity matcher, which can
      // take longer than vitest's five seconds on a busy machine.
      testTimeout: 20_000,
      // obscenity ships CommonJS behind an ESM wrapper. wrangler's bundler joins the two when it
      // deploys; the test runner loads modules one by one, so it gets them pre-bundled instead.
      deps: { optimizer: { ssr: { enabled: true, include: ["obscenity"] } } },
    },
  };
});
