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

/** What `read` finds on the canvas once it has settled: the folder in, and drawn the same twice running. */
async function settled(page: Page, read: () => Promise<string>) {
  await expect(composer(page).locator(".cmp-stage")).not.toHaveAttribute("data-waiting");
  let last: string | null = null;
  await expect
    .poll(async () => {
      const now = await read();
      const same = now === last;
      last = now;
      return same;
    })
    .toBe(true);
  return last!;
}

/** Where the selected layer is, from its X and Y in the settings. */
async function layerAt(page: Page) {
  return { x: Number(await side(page).getByLabel("X", { exact: true }).inputValue()), y: Number(await side(page).getByLabel("Y", { exact: true }).inputValue()) };
}

/** Drags on the canvas from a point in the design's units (0 to 1024 across) by `dx`, `dy` screen pixels. */
async function dragOnCanvas(page: Page, from: { x: number; y: number }, dx: number, dy: number) {
  const box = (await composer(page).locator("canvas.cmp-canvas").boundingBox())!;
  const sx = box.x + (from.x / 1024) * box.width;
  const sy = box.y + (from.y / 1024) * box.height;
  await page.mouse.move(sx, sy);
  await page.mouse.down();
  await page.mouse.move(sx + dx / 2, sy + dy / 2, { steps: 4 });
  await page.mouse.move(sx + dx, sy + dy, { steps: 4 });
  await page.mouse.up();
}

/** A pack of one logo with its brand's colour, served where the preview downloads Simple Icons from. */
const LOGO_PACK = {
  format: 1,
  id: "simple-icons",
  name: "Simple Icons",
  version: "1",
  license: "CC0-1.0",
  source: "https://simpleicons.org",
  style: "fill",
  viewBox: 24,
  brands: true,
  icons: [{ n: "red-square", d: ["M2 2h20v20H2z"], c: "#ff0000" }],
};

/** Holds requests until `open` is called. */
function gate() {
  let open = () => {};
  const wait = new Promise<void>((resolve) => (open = resolve));
  return { wait, open: () => open() };
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

test.describe("the Mac's and Windows' own folders", () => {
  test("start empty on their own folder, with a front of its own colour that covers only the front", async ({ page }) => {
    await openApp(page);
    await openView(page, /design your own/i);
    // They're the folders as each system draws them, with nothing on them: empty starts, not templates.
    const empty = newDialog(page).getByRole("region", { name: "start empty" });
    await expect(empty.getByRole("button")).toHaveText([/^Empty folder/, /^Free icon/, /^Mac folder/, /^Windows folder/]);
    await expect(newDialog(page).getByRole("region", { name: "start from a template" }).getByRole("button", { name: /Mac folder|Windows folder/ })).toHaveCount(0);
    await empty.getByRole("button", { name: /^Windows folder/ }).click();
    await expect(newDialog(page)).toBeHidden();
    const which = composer(page).getByRole("radiogroup", { name: "which folder" });
    await expect(which.getByRole("radio", { name: "Windows" })).toHaveAttribute("aria-checked", "true");
    await expect(layerNames(page)).toHaveText(["Bottom edge", "Front", "Back"]);
    // Nothing new is saved until it is, so a new design doesn't say so.
    await expect(side(page).locator(".cmp-side-sub")).toHaveCount(0);

    // The tab keeps the back's colour; the front's covering the whole folder paints it too.
    const canvas = composer(page).locator("canvas.cmp-canvas");
    const tab = () =>
      canvas.evaluate((c: HTMLCanvasElement) => {
        const px = c.getContext("2d")!.getImageData(Math.round((200 / 1024) * c.width), Math.round((160 / 1024) * c.height), 1, 1).data;
        return `${px[0]},${px[1]},${px[2]}`;
      });
    await side(page).locator(".cmp-layer", { hasText: "Front" }).click();
    const covers = side(page).getByRole("radiogroup", { name: "what the colour covers" });
    await expect(covers.getByRole("radio", { name: "Front" })).toHaveAttribute("aria-checked", "true");
    const golden = await settled(page, tab);
    await covers.getByRole("radio", { name: "Whole folder" }).click();
    await expect.poll(tab).not.toBe(golden);
    await covers.getByRole("radio", { name: "Front" }).click();
    await expect.poll(tab).toBe(golden);

    // From Windows' folder, the Mac's own look still starts on the Mac's.
    await composer(page).getByRole("button", { name: "New" }).click();
    await newDialog(page).getByRole("region", { name: "start empty" }).getByRole("button", { name: /^Mac folder/ }).click();
    await expect(newDialog(page)).toBeHidden();
    await expect(which.getByRole("radio", { name: "Mac" })).toHaveAttribute("aria-checked", "true");
    await expect(layerNames(page)).toHaveText(["Front", "Back"]);
  });
});

test.describe("a new session", () => {
  test("keeps the design while the app runs, and starts afresh the next time it opens", async ({ page, context }) => {
    await openApp(page);
    await startFrom(page, "Label");
    await composer(page).getByRole("button", { name: "Text" }).click();
    await expect(layerNames(page)).toHaveText(["Your words", "Projects", "Background"]);
    // Away to the library and back, and even a reload: the same run, the same design.
    await openView(page, /all skins/i);
    await openView(page, /design your own/i);
    await expect(layerNames(page)).toHaveText(["Your words", "Projects", "Background"]);
    // The design is kept a moment after its last change.
    await expect.poll(() => page.evaluate(() => sessionStorage.getItem("folderskin.composer.draft.v1") ?? "")).toContain("Your words");
    await page.reload();
    await openView(page, /design your own/i);
    await expect(layerNames(page)).toHaveText(["Your words", "Projects", "Background"]);
    // The app opened again (a new window, as a relaunch is): the new-design dialog, not the old one.
    const next = await context.newPage();
    await openApp(next);
    await openView(next, /design your own/i);
    await expect(newDialog(next)).toBeVisible();
    await expect(newDialog(next).getByText("Your current design isn't saved.")).toHaveCount(0);
  });
});

test.describe("what's behind the folder", () => {
  test("is white in light mode and dark in dark mode, and a pick lasts only as long as the app runs", async ({ page, context }) => {
    await openApp(page);
    await startFrom(page, "Label");
    const stage = composer(page).locator(".cmp-stage");
    await expect(stage).toHaveAttribute("data-backdrop", "light");
    await expect(stage).toHaveCSS("background-color", "rgb(255, 255, 255)");
    // Dark mode: the canvas follows.
    await page.getByRole("switch", { name: "dark mode" }).click();
    await expect(stage).toHaveAttribute("data-backdrop", "dark");
    await page.getByRole("switch", { name: "dark mode" }).click();
    await expect(stage).toHaveAttribute("data-backdrop", "light");
    // A backdrop picked stays for this run, whatever the theme does.
    await composer(page).getByRole("radiogroup", { name: "what's behind the folder" }).getByRole("radio", { name: "Colourful wallpaper" }).click();
    await expect(stage).toHaveAttribute("data-backdrop", "colour");
    // Next launch: back to the theme's.
    const next = await context.newPage();
    await openApp(next);
    await startFrom(next, "Label");
    await expect(composer(next).locator(".cmp-stage")).toHaveAttribute("data-backdrop", "light");
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

  test("adds several icons side by side on Windows' folder too", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("radiogroup", { name: "which folder" }).getByRole("radio", { name: "Windows" }).click();
    await composer(page).getByRole("button", { name: "Icon" }).click();
    for (const name of ["Camera", "Aperture", "Anchor"]) {
      await side(page).getByLabel("search icons").fill(name.toLowerCase());
      await side(page).getByRole("button", { name, exact: true }).dblclick();
    }
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    const boxes: { x: number; y: number; w: number }[] = [];
    for (const name of ["Camera", "Aperture", "Anchor"]) {
      await side(page).locator(".cmp-layer", { hasText: name }).click();
      boxes.push({ ...(await layerAt(page)), w: Number(await side(page).getByLabel("Size, typed", { exact: true }).inputValue()) });
    }
    // None on top of another: each pair is at least their half-widths apart across.
    for (const [i, a] of boxes.entries()) for (const b of boxes.slice(i + 1)) expect(Math.abs(a.x - b.x) * 2).toBeGreaterThanOrEqual(a.w + b.w);
  });

  test("new words and shapes on Two-tone stand out from its dark front, not the yellow under it", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Two-tone");
    await composer(page).getByRole("button", { name: "Text" }).click();
    await expect(side(page).getByRole("button", { name: "Text colour: #FFFFFF" })).toBeVisible();
    await composer(page).getByRole("button", { name: "Shape" }).click();
    await page.locator(".cmp-grid-btn[data-tip='Heart']").click();
    await expect(side(page).getByRole("button", { name: "Shape colour: #FFFFFF" })).toBeVisible();
  });

  test("new words go beside the label's own, not over them", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Label");
    await side(page).locator(".cmp-layer", { hasText: "Projects" }).click();
    const label = await layerAt(page);
    const labelSize = Number(await side(page).getByLabel("Size, typed", { exact: true }).inputValue());
    await composer(page).getByRole("button", { name: "Text" }).click();
    await expect(side(page).locator(".cmp-layer.is-on .cmp-layer-label")).toHaveText("Your words");
    const words = await layerAt(page);
    const wordsSize = Number(await side(page).getByLabel("Size, typed", { exact: true }).inputValue());
    // At least a line of each apart, up or down: the words go beside the label, not over it.
    expect(Math.abs(words.y - label.y) * 2).toBeGreaterThanOrEqual(labelSize + wordsSize);
  });

  for (const look of ["Mac", "Windows"]) {
    test(`new words, a shape and an emoji go beside what's there on the ${look} folder, not over it`, async ({ page }) => {
      await openApp(page);
      await startFrom(page, "Label");
      if (look === "Windows") await composer(page).getByRole("radiogroup", { name: "which folder" }).getByRole("radio", { name: "Windows" }).click();
      const words = async (name: string) => {
        await side(page).locator(".cmp-layer", { hasText: name }).click();
        const size = Number(await side(page).getByLabel("Size, typed", { exact: true }).inputValue());
        // Words are wide: what goes beside them goes above or below, a line apart.
        return { ...(await layerAt(page)), w: 2048, h: size * 1.1 };
      };
      await composer(page).getByRole("button", { name: "Text" }).click();
      await expect(side(page).locator(".cmp-layer.is-on .cmp-layer-label")).toHaveText("Your words");
      await composer(page).getByRole("button", { name: "Shape" }).click();
      await page.locator(".cmp-grid-btn[data-tip='Heart']").click();
      const heart = { ...(await layerAt(page)), w: Number(await side(page).getByLabel("W", { exact: true }).inputValue()), h: Number(await side(page).getByLabel("H", { exact: true }).inputValue()) };
      await composer(page).getByRole("button", { name: "Emoji" }).click();
      await page.locator(".cmp-emoji-btn").first().click();
      const size = Number(await side(page).getByLabel("Size, typed", { exact: true }).inputValue());
      const emoji = { ...(await layerAt(page)), w: size, h: size };
      const boxes = [await words("Projects"), await words("Your words"), heart, emoji];
      for (const [i, a] of boxes.entries())
        for (const b of boxes.slice(i + 1)) expect(Math.abs(a.x - b.x) * 2 >= a.w + b.w || Math.abs(a.y - b.y) * 2 >= a.h + b.h, JSON.stringify([a, b])).toBe(true);
    });
  }

  test("tries the icon pointed at on the canvas itself, and puts it back after", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    const canvas = composer(page).locator("canvas.cmp-canvas");
    const picture = () => canvas.evaluate((c: HTMLCanvasElement) => c.toDataURL());
    const before = await settled(page, picture);
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

  test("a layer dragged while an icon is tried moves, and the icon tried stays only tried", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Label");
    await side(page).locator(".cmp-layer", { hasText: "Projects" }).click();
    const before = await layerAt(page);
    await composer(page).getByRole("button", { name: "Icon" }).click();
    await side(page).getByLabel("search icons").fill("camera");
    await side(page).getByRole("button", { name: "Camera", exact: true }).click();
    await expect(composer(page).locator(".cmp-pending")).toBeVisible();
    await dragOnCanvas(page, before, 30, 20);
    // Still only tried: Add to canvas keeps it once.
    await side(page).getByRole("button", { name: "Add to canvas" }).click();
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await expect(layerNames(page)).toHaveText(["Camera", "Projects", "Background"]);
    await side(page).locator(".cmp-layer", { hasText: "Projects" }).click();
    expect(await layerAt(page)).not.toEqual(before);
  });

  test("with an icon selected, dragging it keeps it the icon it is", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    await side(page).getByLabel("search icons").fill("camera");
    await side(page).getByRole("button", { name: "Camera", exact: true }).dblclick();
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await side(page).locator(".cmp-layer", { hasText: "Camera" }).click();
    const at = await layerAt(page);
    await side(page).getByRole("button", { name: "Replace", exact: true }).click();
    // The arrow keys put another icon on the canvas in its place, to look at.
    const search = side(page).getByLabel("search icons");
    await search.fill("");
    await search.press("ArrowDown");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await dragOnCanvas(page, at, 30, 20);
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await expect(layerNames(page)).toHaveText(["Camera", "Background"]);
  });

  test("the arrow keys move through the grid and leave the selected layer where it is", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Label");
    await side(page).locator(".cmp-layer", { hasText: "Projects" }).click();
    const before = await layerAt(page);
    await composer(page).getByRole("button", { name: "Icon" }).click();
    // The icons come in a moment after the library opens; the keys work on them once they have.
    await expect(side(page).locator(".icon-cell").first()).toBeVisible();
    await side(page).getByLabel("search icons").press("ArrowDown");
    await expect(side(page).locator(".icon-cell.is-active")).toBeFocused();
    for (const key of ["ArrowRight", "ArrowRight", "ArrowRight", "ArrowDown"]) await page.keyboard.press(key);
    await expect(side(page).locator(".icon-cell.is-active")).toBeFocused();
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await side(page).locator(".cmp-layer", { hasText: "Projects" }).click();
    expect(await layerAt(page)).toEqual(before);
  });

  test("adds logos in their own colours when Original is chosen", async ({ page }) => {
    await page.route("**/dist-icons/simple-icons.json", (route) => route.fulfill({ contentType: "application/json", body: JSON.stringify(LOGO_PACK) }));
    await openApp(page);
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    await side(page).getByRole("button", { name: /icon pack: Lucide/ }).click();
    await page.getByRole("button", { name: "download Simple Icons" }).click();
    await expect(side(page).getByRole("button", { name: /icon pack: Simple Icons/ })).toBeVisible({ timeout: 10_000 });
    const original = side(page).getByRole("radio", { name: "Original" });
    await original.click();
    await expect(original).toHaveAttribute("aria-checked", "true");
    await side(page).getByRole("button", { name: "Red square", exact: true }).dblclick();
    // Lucide's icons have no colours of their own: they're added flat, and the switch says so.
    await side(page).getByRole("button", { name: /icon pack: Simple Icons/ }).click();
    await page.getByRole("button", { name: /^Lucide/ }).click();
    await expect(side(page).getByRole("radio", { name: "Original" })).toHaveCount(0);
    await expect(side(page).getByRole("radio", { name: "Flat" })).toHaveAttribute("aria-checked", "true");
    await side(page).getByRole("radio", { name: /^Layers/ }).click();
    await side(page).locator(".cmp-layer", { hasText: "Red square" }).click();
    await expect(side(page).getByRole("radio", { name: "Original" })).toHaveAttribute("aria-checked", "true");
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

  test("a pack that can't be downloaded says so over the pack list, not under it", async ({ page }) => {
    // The app's own window size, where the list reaches down to where toasts show.
    await page.setViewportSize({ width: 1125, height: 687 });
    await openApp(page, { query: "offline" });
    await startFrom(page, "Plain");
    await composer(page).getByRole("button", { name: "Icon" }).click();
    await side(page).getByRole("button", { name: /icon pack: Lucide/ }).click();
    await page.getByRole("button", { name: "download Heroicons", exact: true }).click();
    const toast = page.locator(".toasts").getByText(/Couldn't download Heroicons/);
    await expect(toast).toBeVisible();
    // Every part of it is on top, the popover included where they cross.
    const box = (await toast.boundingBox())!;
    for (const x of [box.x + 4, box.x + box.width / 2, box.x + box.width - 4]) {
      const top = await page.evaluate(([px, py]) => document.elementFromPoint(px, py)?.closest(".toasts") !== null, [x, box.y + box.height / 2]);
      expect(top, `at x ${Math.round(x)}`).toBe(true);
    }
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

  test("switches between a Mac's folder and Windows', and straight back puts the design back exactly", async ({ page }) => {
    await openApp(page);
    await startFrom(page, "Tab label");
    const which = composer(page).getByRole("radiogroup", { name: "which folder" });
    const canvas = composer(page).locator("canvas.cmp-canvas");
    const picture = () => canvas.evaluate((c: HTMLCanvasElement) => c.toDataURL());
    const at = async () => [await side(page).getByLabel("X", { exact: true }).inputValue(), await side(page).getByLabel("Y", { exact: true }).inputValue()];
    await which.getByRole("radio", { name: "Mac" }).click();
    await side(page).locator(".cmp-layer", { hasText: "IDEAS" }).click();
    const onMac = await at();
    const macPicture = await settled(page, picture);

    await which.getByRole("radio", { name: "Windows" }).click();
    await expect(which.getByRole("radio", { name: "Windows" })).toHaveAttribute("aria-checked", "true");
    await expect.poll(picture).not.toBe(macPicture);
    // The tab is somewhere else on Windows' folder, and the words went with it.
    expect(await at()).not.toEqual(onMac);
    await which.getByRole("radio", { name: "Mac" }).click();
    expect(await at()).toEqual(onMac);

    await page.keyboard.press("Control+z");
    await expect(which.getByRole("radio", { name: "Windows" })).toHaveAttribute("aria-checked", "true");
    // A free icon has no folder to choose.
    await composer(page).getByRole("switch", { name: "folder skeleton" }).click();
    await expect(which).toHaveCount(0);
  });

  test("shows a loader, not a half-drawn stage, while the folder is on its way", async ({ page }) => {
    // The folders' pictures are held back until each is let through.
    const held = { mac: gate(), windows: gate() };
    await page.route(/\/docs\/images\/composer\/(windows\/)?[a-z]+\.png/, async (route) => {
      await held[route.request().url().includes("/windows/") ? "windows" : "mac"].wait;
      await route.continue();
    });
    await openApp(page);
    await startFrom(page, "Label");
    const loader = composer(page).getByRole("status").filter({ hasText: "Getting the folder ready" });
    const canvas = composer(page).locator("canvas.cmp-canvas");
    await expect(loader).toBeVisible();
    await expect(canvas).toBeHidden();

    const which = composer(page).getByRole("radiogroup", { name: "which folder" });
    const onMac = (await which.getByRole("radio", { name: "Mac" }).getAttribute("aria-checked")) === "true";
    held[onMac ? "mac" : "windows"].open();
    await expect(loader).toBeHidden();
    await expect(canvas).toBeVisible();

    // The other folder, the first time it's chosen.
    await which.getByRole("radio", { name: onMac ? "Windows" : "Mac" }).click();
    await expect(loader).toBeVisible();
    await expect(canvas).toBeHidden();
    held[onMac ? "windows" : "mac"].open();
    await expect(loader).toBeHidden();
    await expect(canvas).toBeVisible();
  });

  test("keeps the canvas its size when the icon at its real sizes comes in under it", async ({ page }) => {
    // The previews aren't drawn until they're let through.
    await page.addInitScript(() => {
      const toBlob = HTMLCanvasElement.prototype.toBlob;
      let open = false;
      const held: (() => void)[] = [];
      (window as unknown as { letThrough: () => void }).letThrough = () => {
        open = true;
        held.splice(0).forEach((go) => go());
      };
      HTMLCanvasElement.prototype.toBlob = function (this: HTMLCanvasElement, ...args: Parameters<HTMLCanvasElement["toBlob"]>) {
        if (open) toBlob.apply(this, args);
        else held.push(() => toBlob.apply(this, args));
      };
    });
    await openApp(page);
    await startFrom(page, "Plain");
    const size = () => composer(page).locator("canvas.cmp-canvas").evaluate((c: HTMLCanvasElement) => `${c.style.width} ${c.style.height}`);
    await expect(composer(page).locator(".cmp-stage")).not.toHaveAttribute("data-waiting");
    const before = await size();
    await page.evaluate(() => (window as unknown as { letThrough: () => void }).letThrough());
    await expect(composer(page).locator(".cmp-size")).toHaveCount(3);
    expect(await size()).toBe(before);
  });

  test("shows a loader, not the design without its photo, while the photo is still being read", async ({ page }) => {
    // The photo (a big JPEG) isn't read until it's let through, as a big one takes a while.
    await page.addInitScript(() => {
      const src = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, "src")!;
      const held: [HTMLImageElement, string][] = [];
      (window as unknown as { letThrough: () => void }).letThrough = () => held.splice(0).forEach(([img, v]) => src.set!.call(img, v));
      Object.defineProperty(HTMLImageElement.prototype, "src", {
        get() {
          return src.get!.call(this);
        },
        set(v: string) {
          if (v.startsWith("data:image/jpeg") && v.length > 20_000) held.push([this, v]);
          else src.set!.call(this, v);
        },
      });
    });
    await openApp(page);
    await startFrom(page, "Photo");
    const loader = composer(page).getByRole("status").filter({ hasText: "Getting the picture ready" });
    const canvas = composer(page).locator("canvas.cmp-canvas");
    await expect(loader).toBeVisible();
    await expect(canvas).toBeHidden();
    await page.evaluate(() => (window as unknown as { letThrough: () => void }).letThrough());
    await expect(loader).toBeHidden();
    await expect(canvas).toBeVisible();
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

  test("the canvas bar gives way rather than drawing its previews or backdrops cut off", async ({ page }) => {
    // The app's own window size, where the previews ran past the panel's edge.
    await page.setViewportSize({ width: 1125, height: 687 });
    await openApp(page);
    await startFrom(page, "Plain");
    const bar = composer(page).locator(".cmp-bar");
    const whole = async () => {
      const edge = (await bar.boundingBox())!;
      for (const piece of await bar.locator(":scope > :visible").all()) {
        const box = (await piece.boundingBox())!;
        expect(box.x + box.width, await piece.evaluate((e) => e.className)).toBeLessThanOrEqual(edge.x + edge.width + 0.5);
      }
      for (const img of await bar.locator(".cmp-size:visible").all()) {
        const box = (await img.boundingBox())!;
        expect(box.x + box.width).toBeLessThanOrEqual(edge.x + edge.width + 0.5);
      }
    };
    await whole();
    // Narrower still, with the sidebar at its widest: the backdrops give way too, never cut.
    await page.getByRole("separator", { name: "sidebar width" }).press("End");
    await whole();
    await expect(bar.locator(".cmp-sizes")).toBeHidden();
    // And back as the room comes back.
    await page.getByRole("separator", { name: "sidebar width" }).press("Home");
    await page.setViewportSize({ width: 1600, height: 900 });
    await expect(bar.locator(".cmp-sizes")).toBeVisible();
    await expect(bar.getByRole("radiogroup", { name: "what's behind the folder" })).toBeVisible();
    await whole();
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
