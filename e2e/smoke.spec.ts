import { expect, test } from "@playwright/test";
import { openApp, openView } from "./app";

test("the library opens with the user's own skins", async ({ page }) => {
  await openApp(page);
  await expect(page.getByRole("button", { name: /options for mona lisa/i })).toBeAttached();
});

test("every view in the sidebar opens", async ({ page }) => {
  await openApp(page);
  await openView(page, /design your own/i);
  await expect(page.getByRole("heading", { name: /design a skin/i })).toBeVisible();
  await openView(page, /generate with ai/i);
  await expect(page.getByRole("heading", { name: /what should your folder look like/i })).toBeVisible();
  await openView(page, /community/i);
  await expect(page.getByRole("heading", { name: /^community$/i })).toBeVisible();
});
