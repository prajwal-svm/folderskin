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
    await expect(side(page).getByLabel("text on the folder", { exact: true })).toHaveValue("Projects");
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
    await side(page).getByLabel("text on the folder", { exact: true }).fill("Archive");
    await expect(layerNames(page).first()).toHaveText("Heading");
    await expect(side(page).locator(".cmp-layer-content").first()).toHaveText("“Archive”");
  });
});

test.describe("the icon library", () => {
  test("tries an icon on the folder, and adds it pressed in with Add to canvas", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    const search = side(page).getByLabel("search icons");
    await expect(search).toHaveAttribute("placeholder", /Search [\d,]+ icons/);
    await search.fill("camera");
    await side(page).getByRole("button", { name: "Camera", exact: true }).click();
    // Tried, outlined on the canvas as not added yet, and named with Add under the grid.
    await expect(composer(page).locator(".cmp-pending")).toBeVisible();
    await expect(side(page).locator(".icon-status strong")).toHaveText("Camera");
    await side(page).getByRole("button", { name: "Add to canvas" }).click();
    await expect(composer(page).locator(".cmp-pending")).toHaveCount(0);
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

  test("trying another icon changes only the one being tried", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    await side(page).getByLabel("search icons").fill("camera");
    await side(page).getByRole("button", { name: "Camera", exact: true }).click();
    await side(page).getByLabel("search icons").fill("aperture");
    await side(page).getByRole("button", { name: "Aperture", exact: true }).click();
    await side(page).getByRole("button", { name: "Add to canvas" }).click();
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await expect(layerNames(page)).toHaveText(["Aperture", "Background"]);
  });

  test("adds several icons, side by side rather than on top of each other", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    for (const name of ["Camera", "Aperture", "Anchor"]) {
      await side(page).getByLabel("search icons").fill(name.toLowerCase());
      // A double-click adds straight away.
      await side(page).getByRole("button", { name, exact: true }).dblclick();
    }
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await expect(layerNames(page)).toHaveText(["Anchor", "Aperture", "Camera", "Background"]);
    const spots = new Set<string>();
    for (const name of ["Camera", "Aperture", "Anchor"]) {
      await side(page).locator(".cmp-layer", { hasText: name }).click();
      spots.add(`${await side(page).getByLabel("X", { exact: true }).inputValue()},${await side(page).getByLabel("Y", { exact: true }).inputValue()}`);
    }
    expect(spots.size).toBe(3);
  });

  test("tries the icon pointed at on the canvas itself, and puts it back after", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    const canvas = composer(page).locator("canvas.cmp-canvas");
    const picture = () => canvas.evaluate((c: HTMLCanvasElement) => c.toDataURL());
    await page.waitForTimeout(400);
    const before = await picture();
    await side(page).getByLabel("search icons").fill("camera");
    await side(page).getByRole("button", { name: "Camera", exact: true }).hover();
    await expect.poll(picture).not.toBe(before);
    await page.mouse.move(5, 5);
    await expect.poll(picture).toBe(before);
    // Nothing was added by looking.
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await expect(layerNames(page)).toHaveText(["Background"]);
  });

  test("with an icon selected, a click swaps it, and Done goes back to adding", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    await side(page).getByLabel("search icons").fill("camera");
    await side(page).getByRole("button", { name: "Camera", exact: true }).dblclick();
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await side(page).locator(".cmp-layer", { hasText: "Camera" }).click();
    await side(page).getByRole("button", { name: "Replace", exact: true }).click();
    await expect(side(page).locator(".icon-status")).toContainText("Click an icon to put it in its place.");
    await side(page).getByLabel("search icons").fill("aperture");
    await side(page).getByRole("button", { name: "Aperture", exact: true }).click();
    await side(page).getByRole("button", { name: "Done", exact: true }).click();
    // Back to adding: a click tries an icon rather than swapping.
    await side(page).getByRole("button", { name: "Aperture", exact: true }).click();
    await expect(side(page).getByRole("button", { name: "Add to canvas" })).toBeVisible();
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await expect(layerNames(page)).toHaveText(["Aperture", "Background"]);
  });

  test("its look switch changes the selected icon on the folder", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    await side(page).getByLabel("search icons").fill("camera");
    await side(page).getByRole("button", { name: "Camera", exact: true }).dblclick();
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await side(page).locator(".cmp-layer", { hasText: "Camera" }).click();
    await side(page).getByRole("radio", { name: "Icons" }).click();
    await side(page).getByRole("radio", { name: "Flat" }).click();
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await expect(side(page).getByRole("radio", { name: "Flat" })).toHaveAttribute("aria-checked", "true");
    // Undo takes the look back, as it would any change to the design.
    await page.keyboard.press("Control+z");
    await expect(side(page).getByRole("radio", { name: "Pressed in" })).toHaveAttribute("aria-checked", "true");
  });

  test("shows each pack with its logo and no licence small print", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    await side(page).getByRole("button", { name: /icon pack: Lucide/ }).click();
    const packs = page.getByRole("list", { name: "icon packs" });
    await expect(packs.locator(".pack-logo svg")).toHaveCount(10);
    await expect(packs).not.toContainText(/ISC|MIT|licen[cs]e/i);
    await expect(side(page).locator(".icon-library")).not.toContainText(/ISC|MIT|licen[cs]e/i);
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

test.describe("the canvas and its panels", () => {
  test("the folder skeleton is the design's shape, and undoable", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    const skeleton = composer(page).getByRole("switch", { name: "folder skeleton" });
    await expect(skeleton).toHaveAttribute("aria-checked", "true");
    await skeleton.click();
    await expect(skeleton).toHaveAttribute("aria-checked", "false");
    // There is no second control for the same thing in the panel.
    await expect(side(page).getByRole("radio", { name: /on the folder|free icon/i })).toHaveCount(0);
    await page.keyboard.press("Control+z");
    await expect(skeleton).toHaveAttribute("aria-checked", "true");
  });

  test("the second panel is Attributes, with tips behind the info button", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Label");
    await expect(side(page).locator(".cmp-panel-title")).toHaveText(["Layers", "Attributes"]);
    await expect(side(page).getByText(/Tips/)).toHaveCount(0);
    await composer(page).getByRole("button", { name: "tips and shortcuts" }).click();
    await expect(page.getByRole("dialog", { name: "Tips and shortcuts" })).toContainText("Click the folder to change its colour.");
    await page.keyboard.press("Escape");
    await side(page).locator(".cmp-layer", { hasText: "Projects" }).click();
    await expect(side(page).locator(".cmp-panel-title")).toHaveText(["Layers", "Attributes"]);
  });

  test("a layer's delete shows on hover with a tooltip, and there's no Delete all", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Label");
    await expect(side(page).getByText("Delete all")).toHaveCount(0);
    const row = side(page).locator(".cmp-layer", { hasText: "Background" });
    const del = row.getByRole("button", { name: "delete Background" });
    await expect(del).toHaveCSS("opacity", "0");
    await row.hover();
    await expect(del).toHaveCSS("opacity", "1");
    await del.hover();
    await expect(page.getByRole("tooltip")).toHaveText("Delete layer");
    await del.click();
    await expect(layerNames(page)).toHaveText(["Projects"]);
  });

  test("blend is chosen from the app's own dropdown", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Label");
    await side(page).locator(".cmp-layer", { hasText: "Projects" }).click();
    await side(page).getByRole("button", { name: /^blend: Normal/ }).click();
    const list = page.getByRole("listbox", { name: "blend" });
    await expect(list.getByRole("option", { name: "Normal" })).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Enter");
    await expect(side(page).getByRole("button", { name: /^blend: Multiply/ })).toBeVisible();
    await expect(page.locator("select")).toHaveCount(0);
  });

  test("tools that don't fit go into More", async ({ page }) => {
    await page.setViewportSize({ width: 1040, height: 700 });
    await openApp(page);
    await startFrom(page, "Label");
    const tools = composer(page).getByRole("toolbar", { name: "add to the design" });
    await tools.getByRole("button", { name: "more tools" }).click();
    const menu = page.getByRole("menu");
    await expect(menu.getByRole("menuitem", { name: "Colour" })).toBeVisible();
    // Each tool is in the toolbar or in the menu, never both, and never cut in half.
    const inline = await tools.locator(":scope > .cmp-tool:not([aria-label='more tools'])").allInnerTexts();
    const inMenu = await menu.getByRole("menuitem").allInnerTexts();
    expect([...inline, ...inMenu].map((t) => t.trim()).sort()).toEqual(["Colour", "Emoji", "Icon", "Pattern", "Picture", "Shape", "Text"]);
    const bar = await tools.boundingBox();
    for (const b of await tools.locator(":scope > .cmp-tool").all()) {
      const box = await b.boundingBox();
      expect(box!.x + box!.width).toBeLessThanOrEqual(bar!.x + bar!.width + 0.5);
    }
    // Colour from the menu does what the tool does: the folder's colour layer is selected.
    await menu.getByRole("menuitem", { name: "Colour" }).click();
    await expect(side(page).locator(".cmp-layer.is-on .cmp-layer-label")).toHaveText("Background");
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
