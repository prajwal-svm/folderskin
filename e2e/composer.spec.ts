import { expect, test, type Page } from "@playwright/test";
import { openApp, openView } from "./app";

const composer = (page: Page) => page.locator('section[aria-label="composer"]:not([hidden])');
const side = (page: Page) => page.locator('aside[aria-label="layers and settings"]:not([hidden])');
const newDialog = (page: Page) => page.getByRole("dialog", { name: "Start a new design" });
const layerNames = (page: Page) => side(page).locator(".cmp-layer-label");

/** Opens the composer on a fresh design made from `template`. */
async function startFrom(page: Page, template: string) {
  await openView(page, /design your own/i);
  await expect(newDialog(page)).toBeVisible();
  await newDialog(page).getByRole("button", { name: new RegExp(`^${template}$`) }).click();
  await expect(newDialog(page)).toBeHidden();
}

test.describe("starting a new design", () => {
  test("opens as a dialog over the whole window on the first visit", async ({ page }) => {
    await openApp(page);
    await openView(page, /design your own/i);
    const dialog = newDialog(page);
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("button", { name: /empty folder/i })).toBeVisible();
    await expect(dialog.getByRole("button", { name: /free icon/i })).toBeVisible();
    // The app behind it can't be reached.
    await expect(page.locator("#root")).toHaveAttribute("inert", "");
    await dialog.getByRole("button", { name: /^Label$/ }).click();
    await expect(dialog).toBeHidden();
    await expect(layerNames(page)).toHaveText(["Projects", "Background"]);
  });

  test("warns about unsaved changes, and nothing reaches the design behind it", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Label");
    await composer(page).getByRole("button", { name: "Text" }).click();
    await expect(layerNames(page)).toHaveText(["Your words", "Projects", "Background"]);

    await composer(page).getByRole("button", { name: "New" }).click();
    const dialog = newDialog(page);
    await expect(dialog.getByText("Your current design isn't saved.")).toBeVisible();
    await expect(dialog.getByRole("button", { name: "Save it first" })).toBeFocused();
    // Undo and delete are the design's shortcuts; with the dialog open they do nothing.
    await page.keyboard.press("Control+z");
    await page.keyboard.press("Delete");
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(layerNames(page)).toHaveText(["Your words", "Projects", "Background"]);

    await composer(page).getByRole("button", { name: "New" }).click();
    await newDialog(page).getByRole("button", { name: /empty folder/i }).click();
    await expect(side(page).getByText(/nothing here yet/i)).toBeVisible();
    await expect(side(page).getByLabel("skin name")).toHaveValue("");
  });
});

test.describe("layer names and words", () => {
  test("renaming a text layer names it in the list and leaves the words alone", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Label");
    const row = side(page).locator(".cmp-layer", { hasText: "Projects" });
    await row.dblclick();
    const input = side(page).getByLabel("layer name").first();
    await input.fill("Title");
    await input.press("Enter");
    // The row shows the name and, beside it, the words on the folder.
    await expect(side(page).locator(".cmp-layer-label").first()).toHaveText("Title");
    await expect(side(page).locator(".cmp-layer-content").first()).toHaveText("“Projects”");
    await side(page).locator(".cmp-layer", { hasText: "Title" }).click();
    await expect(side(page).getByLabel("text on the folder")).toHaveValue("Projects");
  });

  test("leaving a name as it was changes nothing", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Label");
    const undo = composer(page).getByRole("button", { name: "Undo" });
    await expect(undo).toBeDisabled();
    await side(page).locator(".cmp-layer", { hasText: "Projects" }).dblclick();
    await side(page).getByLabel("layer name").first().press("Enter");
    await expect(undo).toBeDisabled();
    await expect(layerNames(page).first()).toHaveText("Projects");
  });

  test("the settings name a layer apart from its words", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Label");
    await side(page).locator(".cmp-layer", { hasText: "Projects" }).click();
    const name = side(page).locator(".cmp-layer-naming").getByLabel("layer name");
    await expect(name).toHaveAttribute("placeholder", "Projects");
    await name.fill("Heading");
    await name.press("Enter");
    await expect(layerNames(page).first()).toHaveText("Heading");
    await side(page).getByLabel("text on the folder").fill("Archive");
    await expect(layerNames(page).first()).toHaveText("Heading");
    await expect(side(page).locator(".cmp-layer-content").first()).toHaveText("“Archive”");
  });
});

test.describe("the icon library", () => {
  test("finds an icon, previews it and adds it pressed into the folder", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    const search = side(page).getByLabel("search icons");
    await expect(search).toHaveAttribute("placeholder", /Search [\d,]+ icons/);
    await search.fill("camera");
    const cell = side(page).getByRole("button", { name: "Camera", exact: true });
    await cell.hover();
    await expect(side(page).locator(".icon-preview-name")).toHaveText("Camera");
    await cell.click();
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await expect(layerNames(page)).toHaveText(["Camera", "Background"]);
    await side(page).locator(".cmp-layer", { hasText: "Camera" }).click();
    await expect(side(page).getByRole("radio", { name: "Pressed in" })).toHaveAttribute("aria-checked", "true");
    await expect(side(page).getByRole("switch", { name: /folder's own colour/i })).toHaveAttribute("aria-checked", "true");
  });

  test("keeps its search when the layers are looked at in between", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    await side(page).getByLabel("search icons").fill("folder");
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await side(page).getByRole("radio", { name: "Icons" }).click();
    await expect(side(page).getByLabel("search icons")).toHaveValue("folder");
  });

  test("replaces an icon in place", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    await side(page).getByLabel("search icons").fill("camera");
    await side(page).getByRole("button", { name: "Camera", exact: true }).click();
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await side(page).locator(".cmp-layer", { hasText: "Camera" }).click();
    await side(page).getByRole("button", { name: "Replace", exact: true }).click();
    await expect(side(page).getByText(/Replacing/)).toBeVisible();
    await side(page).getByLabel("search icons").fill("aperture");
    await side(page).getByRole("button", { name: "Aperture", exact: true }).click();
    // Back in the layers, the same layer wearing the new icon.
    await expect(layerNames(page)).toHaveText(["Aperture", "Background"]);
  });

  test("downloads another pack and switches to it", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    await side(page).getByRole("button", { name: /icon pack: Lucide/ }).click();
    await page.getByRole("button", { name: "download Tabler" }).click();
    await expect(side(page).getByRole("button", { name: /icon pack: Tabler/ })).toBeVisible({ timeout: 10_000 });
    await side(page).getByLabel("search icons").fill("camera");
    await expect(side(page).locator(".icon-cell").first()).toBeVisible();
  });

  test("draws only the icons on screen", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    await expect(side(page).locator(".icon-cell").first()).toBeVisible();
    // Two thousand icons in the pack; a few hundred at most in the page.
    expect(await side(page).locator(".icon-cell").count()).toBeLessThan(400);
  });
});

test("the ⋯ menu is wide enough, centred on its skin, and cuts nothing short", async ({ page }) => {
  await openApp(page);
  const button = page.getByRole("button", { name: "options for Wanderer above the Sea of Fog" });
  await button.hover();
  await button.click();
  const menu = page.locator(".skin-menu");
  await expect(menu).toBeVisible();
  const [m, tile] = await Promise.all([menu.boundingBox(), button.locator("xpath=ancestor::*[contains(@class,'tile')][1]").boundingBox()]);
  expect(m!.width).toBeGreaterThanOrEqual(340);
  expect(Math.abs(m!.x + m!.width / 2 - (tile!.x + tile!.width / 2))).toBeLessThan(2);
  const name = menu.locator(".skin-menu-name");
  await expect(name).toHaveValue("Wanderer above the Sea of Fog");
  const clipped = await name.evaluate((el) => el.scrollHeight > el.clientHeight + 1 || el.scrollWidth > el.clientWidth + 1);
  expect(clipped).toBe(false);
});
