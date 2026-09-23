import { expect, type Page } from "@playwright/test";

/**
 * Opens the app the way a returning user sees it: onboarding already done, and `yours` skins of
 * their own in the library (devMock's `?yours=`). `query` adds more of devMock's switches, such as
 * `offline` or `real`.
 */
export async function openApp(page: Page, { yours = 8, query = "" }: { yours?: number; query?: string } = {}) {
  await page.addInitScript(() => localStorage.setItem("folderskin.mock.onboarded", "1"));
  await page.goto(`/?yours=${yours}${query ? `&${query}` : ""}`);
  await expect(page.getByRole("button", { name: /all skins/i }).first()).toBeVisible();
}

/** Switches views with the sidebar. */
export async function openView(page: Page, name: RegExp) {
  await page.getByRole("button", { name }).first().click();
}
