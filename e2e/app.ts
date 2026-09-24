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

/** Waits until the preview holds something for the test (its `?hold…` switches) at `go`. */
export async function held(page: Page, go: string) {
  await page.waitForFunction((go) => typeof (window as unknown as Record<string, unknown>)[go] === "function", go);
}

/** Lets through what the preview holds at `go`, once it's holding it. */
export async function letGo(page: Page, go: string) {
  await held(page, go);
  await page.evaluate((go) => (window as unknown as Record<string, (() => void) | undefined>)[go]?.(), go);
}

/** Switches views with the sidebar. */
export async function openView(page: Page, name: RegExp) {
  await page.getByRole("button", { name }).first().click();
}
