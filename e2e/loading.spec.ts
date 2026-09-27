import { expect, test, type Page } from "@playwright/test";
import { openApp, openView } from "./app";

// What's still on its way says so, and what the app can get ready ahead, it does: the views'
// code once the library is on show (src/lib/warmUp.ts), and the shapes' pictures with it.
// `?slowshapes` makes the preview take four seconds to draw the shapes, as a slow computer might
// on its first launch.

const chat = (page: Page) => page.locator('section[aria-label="generate with AI"]:not([hidden])');
const box = (page: Page) => chat(page).getByLabel("describe the folder");
const shapeChip = (page: Page) => chat(page).locator(".shape-chip");
const newDialog = (page: Page) => page.getByRole("dialog", { name: "Start a new design" });

test.describe("what's still loading", () => {
  test("the shape chip and its list say the shapes are being drawn, then show them", async ({ page }) => {
    await openApp(page, { query: "slowshapes" });
    await openView(page, /generate with ai/i);
    // The chip is there from the start, on the folder skins go on, a spinner for its picture.
    await expect(shapeChip(page)).toHaveAccessibleName("Mac folder: choose what it's for");
    await expect(shapeChip(page).locator(".shape-thumb svg")).toBeVisible();
    await shapeChip(page).click();
    const shapes = page.getByRole("dialog", { name: "shapes" });
    await expect(shapes.getByRole("status")).toHaveText("Drawing the shapes");
    // As soon as they're in, the list takes the note's place, while it's open.
    await expect(shapes.getByRole("radio", { name: /Mac folder/ })).toBeVisible({ timeout: 8000 });
    await expect(shapes.getByRole("status")).toHaveCount(0);
    await expect(shapeChip(page).locator(".shape-thumb img")).toBeVisible();
  });

  test("@ opens its menu at once, saying the shapes are on their way", async ({ page }) => {
    await openApp(page, { query: "slowshapes" });
    await openView(page, /generate with ai/i);
    await box(page).click();
    await box(page).pressSequentially("@");
    const menu = page.locator(".pm");
    await expect(menu.getByRole("status")).toHaveText("Drawing the shapes");
    await expect(menu.getByRole("option")).toHaveCount(3 + 9 + 6 + 10 + 1, { timeout: 8000 });
  });

  test("a drive's card waits for its picture with a spinner", async ({ page }) => {
    await openApp(page, { query: "slowshapes" });
    await openView(page, /design your own/i);
    await expect(newDialog(page)).toBeVisible();
    const card = newDialog(page).locator(".cmp-card.is-drive").first();
    await expect(card.locator(".cmp-card-wait svg")).toBeVisible();
    await expect(card.locator("img")).toBeVisible({ timeout: 8000 });
  });

  test("the views' code and the shapes are fetched ahead, once the library is on show", async ({ page }) => {
    // Every request the page makes, from the start (the browser's own timing list keeps 250).
    const views = new Set<string>();
    page.on("request", (r) => {
      const view = /\/components\/(studio\/Studio|composer\/Composer|SubfolderChooser)\.tsx/.exec(r.url())?.[1];
      if (view) views.add(view);
    });
    await openApp(page);
    await expect.poll(() => views.size, { timeout: 20_000 }).toBe(3);
    // Opened after that, the AI view has every shape's picture from its first frame.
    await openView(page, /generate with ai/i);
    await expect(shapeChip(page).locator(".shape-thumb img")).toBeVisible();
    await shapeChip(page).click();
    await expect(page.getByRole("dialog", { name: "shapes" }).getByRole("status")).toHaveCount(0);
  });
});
