import { expect, test, type Page } from "@playwright/test";
import { openApp, openView } from "./app";

// The Windows window as devMock draws it with `?os=windows`: no title bar of the system's, the
// app's own minimise, maximise and close over the folder island's top right corner.

const box = async (page: Page, selector: string) => (await page.locator(selector).first().boundingBox())!;
/** The element a click at the middle of `selector` would land on, named by its label or class. */
const hitAt = (page: Page, selector: string) =>
  page.locator(selector).first().evaluate((el) => {
    const b = el.getBoundingClientRect();
    const hit = document.elementFromPoint(b.x + b.width / 2, b.y + b.height / 2);
    return hit?.closest("button")?.getAttribute("aria-label") ?? hit?.className.toString() ?? null;
  });

test.describe("the Windows window", () => {
  test("puts the folder panel's button in line with the library's round buttons, open and closed", async ({ page }) => {
    await openApp(page, { query: "os=windows" });
    const toggle = page.locator(".panel-toggle");
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    const filter = await box(page, ".filter-btn");
    const open = await box(page, ".panel-toggle");
    const panel = await box(page, ".right-slot");
    const library = await box(page, ".island-main");
    // Level with the filter, the same size, and as far in from the panel's left edge as the filter
    // is from the library's right edge.
    expect(open.y).toBeCloseTo(filter.y, 0);
    expect(open.height).toBeCloseTo(filter.height, 0);
    expect(open.x - panel.x).toBeCloseTo(library.x + library.width - (filter.x + filter.width), 0);
    // It's a round button, not one of the window's own.
    await expect(page.locator(".winctl .panel-toggle")).toHaveCount(0);

    await toggle.click();
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    // Once it has slid there: at the end of the toolbar's row, left of the window's buttons.
    await expect.poll(async () => (await box(page, ".panel-toggle")).x - (await box(page, ".filter-btn")).x).toBeCloseTo(46, 0);
    const closed = await box(page, ".panel-toggle");
    const controls = await box(page, ".winctl");
    expect(closed.y).toBeCloseTo(filter.y, 0);
    expect(controls.x - (closed.x + closed.width)).toBeCloseTo(12, 0);
    expect(await hitAt(page, ".panel-toggle")).toBe("open the folder panel");
  });

  test("keeps the window's buttons in reach while a dialog is open", async ({ page }) => {
    await openApp(page, { query: "os=windows" });
    await page.getByRole("button", { name: /settings/i }).first().click();
    await expect(page.getByRole("dialog")).toBeVisible();
    // The app behind the dialog is inert, so the dialog brings its own, over its backdrop.
    await expect(page.locator(".modal-winbar .winctl")).toBeVisible();
    for (const name of ["Minimise", "Close"]) {
      expect(await hitAt(page, `.modal-winbar .winctl button[aria-label="${name}"]`)).toBe(name);
    }
    // And the dialog starts below them.
    expect((await box(page, ".modal")).y).toBeGreaterThanOrEqual((await box(page, ".modal-winbar")).y + 32);
    await page.keyboard.press("Escape");
    await expect(page.locator(".modal-winbar")).toHaveCount(0);
  });

  test("gives the first-launch welcome the window's buttons", async ({ page }) => {
    await page.goto("/?os=windows&onboarding");
    await expect(page.locator(".onboard-winctl .winctl")).toBeVisible();
    expect(await hitAt(page, '.onboard-winctl button[aria-label="Close"]')).toBe("Close");
  });

  test("lets Community's header move the window with the folder panel closed", async ({ page }) => {
    await openApp(page, { query: "os=windows" });
    await openView(page, /community/i);
    await expect(page.locator(".community-head")).toHaveAttribute("data-tauri-drag-region", /.*/);
  });
});
