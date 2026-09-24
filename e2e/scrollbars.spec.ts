import { expect, test } from "@playwright/test";
import { openApp, openView } from "./app";

// Playwright hides scrollbars by default, and then they never take room. These tests are about
// the room they take, so they run with the real ones.
test.use({ launchOptions: { ignoreDefaultArgs: ["--hide-scrollbars"] } });

test("the composer's settings keep their width whether or not they overflow, so nothing shifts", async ({ page }) => {
  await openApp(page);
  await openView(page, /design your own/i);
  const dialog = page.getByRole("dialog", { name: "Start a new design" });
  await dialog.getByRole("button", { name: /^Plain$/ }).click();
  await expect(dialog).toBeHidden();
  const side = page.locator('aside[aria-label="layers and settings"]:not([hidden])');
  await side.locator(".cmp-layer", { hasText: "Background" }).click();
  const settings = side.locator(".cmp-side-scroll");
  const measure = () => settings.evaluate((el) => ({ width: el.clientWidth, overflows: el.scrollHeight > el.clientHeight }));

  await page.setViewportSize({ width: 1400, height: 1500 });
  await expect.poll(async () => (await measure()).overflows).toBe(false);
  const fits = (await measure()).width;
  // Taking the design off the folder hides the colour's Covers row: the column doesn't move.
  const skeleton = page.getByRole("switch", { name: "folder skeleton" });
  await skeleton.click();
  await expect(side.getByRole("radiogroup", { name: "what the colour covers" })).toHaveCount(0);
  expect((await measure()).width).toBe(fits);
  await skeleton.click();
  await expect(side.getByRole("radiogroup", { name: "what the colour covers" })).toHaveCount(1);
  // Short enough that the settings scroll: still the same width.
  await page.setViewportSize({ width: 1400, height: 700 });
  await expect.poll(async () => (await measure()).overflows).toBe(true);
  expect((await measure()).width).toBe(fits);
});
