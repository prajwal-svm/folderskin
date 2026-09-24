import { defineConfig, devices } from "@playwright/test";

// End-to-end tests drive the real interface in a browser, where src/lib/devMock.ts stands in for
// the Tauri commands (the same preview `pnpm dev` gives). E2E_PORT lets two checkouts run their
// suites side by side.
const port = Number(process.env.E2E_PORT ?? 14210);

export default defineConfig({
  testDir: "e2e",
  // A cold dev server compiles the app on the first visit, which a busy machine can take a while
  // over; the tests' own waits are short.
  timeout: 45_000,
  expect: { timeout: 7_000 },
  fullyParallel: true,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["github"], ["list"]] : "list",
  use: {
    baseURL: `http://localhost:${port}`,
    navigationTimeout: 40_000,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1440, height: 900 } } }],
  webServer: {
    command: `node node_modules/vite/bin/vite.js --port ${port} --strictPort`,
    url: `http://localhost:${port}`,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
