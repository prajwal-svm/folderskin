import { expect, test, type Page } from "@playwright/test";
import { openApp, openView } from "./app";

const sidebar = (page: Page) => page.getByRole("navigation", { name: "sections" });
const width = async (page: Page, selector: string) => (await page.locator(selector).first().boundingBox())!.width;

test.describe("the sidebar", () => {
  test("folds to an island of icons and opens again", async ({ page }) => {
    await openApp(page);
    await sidebar(page).getByRole("button", { name: "collapse the sidebar" }).click();
    await expect(sidebar(page)).toHaveClass(/is-rail/);
    // The logo stays; the name and version go.
    await expect(sidebar(page).locator(".brand-mark")).toBeVisible();
    await expect(sidebar(page).locator(".brand-name")).toHaveCount(0);
    await expect(sidebar(page).getByRole("button", { name: "about FolderSkin" })).toHaveCount(0);
    await expect.poll(() => width(page, ".sidebar")).toBeLessThan(80);
    // The icons still work, and say what they are in a tooltip.
    const yours = sidebar(page).getByRole("button", { name: "Yours", exact: true });
    await yours.hover();
    await expect(page.getByRole("tooltip")).toHaveText("Yours · 8");
    await yours.click();
    await expect(yours).toHaveAttribute("aria-current", "page");
    await sidebar(page).getByRole("button", { name: "expand the sidebar" }).click();
    await expect(sidebar(page)).not.toHaveClass(/is-rail/);
    await expect(sidebar(page).locator(".brand-name")).toBeVisible();
  });

  test("folds with the keyboard, and stays as it was left", async ({ page }) => {
    await openApp(page);
    await page.keyboard.press("Control+\\");
    await expect(sidebar(page)).toHaveClass(/is-rail/);
    await page.waitForTimeout(400);
    await page.reload();
    await expect(sidebar(page)).toHaveClass(/is-rail/);
    await page.keyboard.press("Control+\\");
    await expect(sidebar(page)).not.toHaveClass(/is-rail/);
  });

  test("its collapse button sits just above the theme switch", async ({ page }) => {
    await openApp(page);
    const buttons = sidebar(page).locator(".sidebar-bottom > button");
    await expect(buttons.nth(0)).toHaveAccessibleName("collapse the sidebar");
    await expect(buttons.nth(1)).toHaveAccessibleName("dark mode");
  });
});

test.describe("the edges between islands", () => {
  test("show only when pointed at, and drag the island beside them", async ({ page }) => {
    await openApp(page);
    const edge = page.getByRole("separator", { name: "folder panel width" });
    const line = () => edge.evaluate((el) => getComputedStyle(el, "::after").opacity);
    expect(await line()).toBe("0");
    const box = (await edge.boundingBox())!;
    const before = await width(page, ".stage-island");
    await page.mouse.move(box.x + box.width / 2, box.y + 300);
    await expect.poll(line).not.toBe("0");
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2 - 60, box.y + 300, { steps: 4 });
    await page.mouse.up();
    await expect.poll(() => width(page, ".stage-island")).toBeGreaterThan(before + 50);
    // A double click puts it back.
    await edge.dblclick();
    await expect.poll(() => width(page, ".stage-island")).toBeCloseTo(before, 0);
  });

  test("folds the sidebar when it's dragged narrow", async ({ page }) => {
    await openApp(page);
    const edge = page.getByRole("separator", { name: "sidebar width" });
    const box = (await edge.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + 300);
    await page.mouse.down();
    await page.mouse.move(80, box.y + 300, { steps: 6 });
    await page.mouse.up();
    await expect(sidebar(page)).toHaveClass(/is-rail/);
  });

  test("move with the arrow keys", async ({ page }) => {
    await openApp(page);
    const edge = page.getByRole("separator", { name: "sidebar width" });
    await edge.focus();
    const before = Number(await edge.getAttribute("aria-valuenow"));
    await page.keyboard.press("ArrowRight");
    await expect(edge).toHaveAttribute("aria-valuenow", String(before + 16));
  });
});

test.describe("tooltips", () => {
  test("are the app's own, never the system's", async ({ page }) => {
    await openApp(page);
    const views = [/all skins/i, /design your own/i, /generate with ai/i];
    for (const view of views) {
      await openView(page, view);
      if (String(view) === String(/design your own/i)) {
        await expect(page.getByRole("dialog", { name: "Start a new design" })).toBeVisible();
        await page.keyboard.press("Escape");
      }
      await page.waitForTimeout(400);
      const titled = await page.evaluate(() => [...document.querySelectorAll("[title], svg title")].map((e) => e.outerHTML.slice(0, 120)));
      expect(titled, `native tooltips in ${view}`).toEqual([]);
    }
  });

  test("show after a moment on hover, with the shortcut beside them", async ({ page }) => {
    await openApp(page);
    await openView(page, /design your own/i);
    await page.getByRole("dialog", { name: "Start a new design" }).getByRole("button", { name: /^Plain$/ }).click();
    await page.getByRole("button", { name: "tips and shortcuts" }).hover();
    await expect(page.getByRole("tooltip")).toHaveText("Tips and shortcuts");
    await page.mouse.move(5, 400);
    await expect(page.getByRole("tooltip")).toHaveCount(0);
  });

  test("show a cut-short name in full, and a name that fits not at all", async ({ page }) => {
    await openApp(page);
    const long = page.locator(".tile-name", { hasText: "Wanderer" }).first();
    await long.hover();
    await expect(page.getByRole("tooltip")).toHaveText("Wanderer above the Sea of Fog");
    await page.locator(".tile-name", { hasText: /^Mona Lisa$/ }).first().hover();
    await page.waitForTimeout(700);
    await expect(page.getByRole("tooltip")).toHaveCount(0);
  });
});
