import { chromium, type FullConfig } from "@playwright/test";

/**
 * Runs once, before any test: opens the app and each part of it whose code loads on first use
 * (the composer, its icons, the AI view), so the dev server has compiled all of it before a test
 * waits on it. A cold server compiles each part on its first visit, which on a machine busy with
 * the whole suite can take longer than a test's waits.
 */
export default async function warm(config: FullConfig) {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ baseURL: config.projects[0].use.baseURL });
    page.setDefaultTimeout(120_000);
    await page.addInitScript(() => localStorage.setItem("folderskin.mock.onboarded", "1"));
    await page.goto("/?yours=8", { timeout: 120_000 });
    await page.getByRole("button", { name: /all skins/i }).first().waitFor();
    await page.getByRole("button", { name: /design your own/i }).first().click();
    const start = page.getByRole("dialog", { name: "Start a new design" });
    await start.getByRole("button", { name: /^Plain$/ }).click();
    await start.waitFor({ state: "hidden" });
    await page.locator('section[aria-label="composer"]:not([hidden])').getByRole("button", { name: "Icon" }).click();
    await page.locator(".icon-cell").first().waitFor();
    await page.getByRole("button", { name: /generate with ai/i }).first().click();
    await page.locator('section[aria-label="generate with AI"]:not([hidden]) .model-pill').waitFor();
  } finally {
    await browser.close();
  }
}
