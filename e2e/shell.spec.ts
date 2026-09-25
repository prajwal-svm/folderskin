import { expect, test, type Page } from "@playwright/test";
import { letGo, openApp, openView } from "./app";

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
    // Kept a moment after it changes.
    await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem("folderskin.layout") ?? "{}").rail)).toBe(true);
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

  test("fold the sidebar with a step narrower from its narrowest, and open it with a step wider", async ({ page }) => {
    await openApp(page);
    const edge = page.getByRole("separator", { name: "sidebar width" });
    await edge.focus();
    await expect(edge).toHaveAttribute("aria-valuenow", "228");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowLeft");
    await expect(edge).toHaveAttribute("aria-valuenow", "196");
    await expect(sidebar(page)).not.toHaveClass(/is-rail/);
    await page.keyboard.press("ArrowLeft");
    await expect(sidebar(page)).toHaveClass(/is-rail/);
    // The edge keeps the focus while the sidebar folds, so the next key still moves it.
    await expect.poll(() => width(page, ".sidebar")).toBeLessThan(80);
    await expect(edge).toBeFocused();
    // Its value stays within the range it gives.
    await expect(edge).toHaveAttribute("aria-valuenow", "64");
    await expect(edge).toHaveAttribute("aria-valuemin", "64");
    await page.keyboard.press("ArrowLeft");
    await expect(sidebar(page)).toHaveClass(/is-rail/);
    await page.keyboard.press("ArrowRight");
    await expect(sidebar(page)).not.toHaveClass(/is-rail/);
    await expect(edge).toHaveAttribute("aria-valuenow", "196");
    await expect(edge).toBeFocused();
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

  test("are the app's own in Community and the share dialog too", async ({ page }) => {
    const titled = () => page.evaluate(() => [...document.querySelectorAll("[title], svg title")].map((e) => e.outerHTML.slice(0, 120)));
    await openApp(page, { query: "shared" });
    await openView(page, /community/i);
    // A pack added, with a newer version out, so its card has every button it can have.
    const colours = page.locator(".pack").filter({ has: page.locator(".pack-name", { hasText: /^Colours$/ }) });
    await colours.getByRole("button", { name: "Add", exact: true }).click();
    await expect(colours.getByRole("img", { name: "Added to your library" })).toBeVisible({ timeout: 10_000 });
    await page.getByRole("button", { name: "refresh packs" }).click();
    await expect(colours.getByRole("button", { name: "Update", exact: true })).toBeVisible();
    // A search that finds skins by name, and the sort and tags beside it.
    await page.getByRole("searchbox", { name: /search packs/i }).fill("toledo");
    await expect(page.locator(".skin-hit").first()).toBeVisible();
    await page.getByRole("button", { name: /sort and more tags/i }).click();
    await expect(page.getByRole("dialog", { name: /sort and tags/i })).toBeVisible();
    expect(await titled(), "native tooltips in Community").toEqual([]);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: /sort and tags/i })).toHaveCount(0);

    await page.locator(".skin-hit").first().click();
    await expect(page.locator(".pack-skin").first()).toBeVisible();
    expect(await titled(), "native tooltips in a pack").toEqual([]);
    await page.keyboard.press("Escape");

    await page.getByRole("button", { name: "Share your skins" }).click();
    const share = page.getByRole("dialog", { name: "Share a pack" });
    await expect(share.getByText("Verified on this computer")).toBeVisible();
    expect(await titled(), "native tooltips in the share dialog").toEqual([]);
    await share.getByRole("button", { name: "Your submissions" }).click();
    await expect(page.getByRole("dialog", { name: "Your submissions" }).getByRole("listitem").first()).toBeVisible();
    expect(await titled(), "native tooltips in Your submissions").toEqual([]);
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

test.describe("the filters beside a search", () => {
  test("take the focus, so Escape closes them and leaves what was typed", async ({ page }) => {
    await openApp(page);
    const library = page.getByLabel("search skins");
    await library.fill("mona");
    await page.getByRole("button", { name: "Filters", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "filter and sort skins" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: "filter and sort skins" })).toHaveCount(0);
    await expect(library).toHaveValue("mona");

    await openView(page, /community/i);
    const packs = page.getByRole("searchbox", { name: /search packs/i });
    await packs.fill("toledo");
    await page.getByRole("button", { name: /sort and more tags/i }).click();
    await expect(page.getByRole("dialog", { name: /sort and tags/i })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: /sort and tags/i })).toHaveCount(0);
    await expect(packs).toHaveValue("toledo");
    await expect(page.locator(".skin-hit").first()).toBeVisible();
  });
});

test.describe("the folder skins go on", () => {
  test("switching it says the library is being drawn again, and dims the old thumbnails until then", async ({ page }) => {
    // Each redraw lasts until it's let through, so the note is looked at while it's there.
    await openApp(page, { query: "holdredraw" });
    const redrawn = () => letGo(page, "mockRedrawn");
    const look = page.getByRole("radiogroup", { name: "which folder skins go on" });
    const note = page.getByRole("status").filter({ hasText: "Drawing your skins on Windows' folder" });
    const gallery = page.locator(".gallery-scroll");
    await look.getByRole("radio", { name: "Windows" }).click();
    await expect(note).toBeVisible();
    await expect(gallery).toHaveAttribute("aria-busy", "true");
    // In the middle of the library, and all of it inside.
    const [at, library] = await Promise.all([note.boundingBox(), page.locator(".island-main").boundingBox()]);
    expect(Math.abs(at!.x + at!.width / 2 - (library!.x + library!.width / 2))).toBeLessThan(2);
    expect(at!.x).toBeGreaterThan(library!.x);
    expect(at!.x + at!.width).toBeLessThan(library!.x + library!.width);
    await redrawn();
    await expect(note).toBeHidden();
    await expect(gallery).not.toHaveAttribute("aria-busy", "true");
    // And back, which says so too.
    await look.getByRole("radio", { name: "Mac" }).click();
    await expect(page.getByRole("status").filter({ hasText: "Drawing your skins on the Mac's folder" })).toBeVisible();
    await redrawn();
    await expect(gallery).not.toHaveAttribute("aria-busy", "true");
  });

  test("its switch comes back once the skin being tried is put down", async ({ page }) => {
    await openApp(page);
    const look = page.getByRole("radiogroup", { name: "which folder skins go on" });
    const tile = page.locator(".tile-hit").first();
    const waiting = page.getByRole("heading", { name: "Now drop a folder" });
    await expect(look).toBeVisible();

    // Escape puts it down.
    await tile.click();
    await expect(waiting).toBeVisible();
    await expect(look).toHaveCount(0);
    await page.keyboard.press("Escape");
    await expect(look).toBeVisible();
    await expect(tile).toHaveAttribute("aria-pressed", "false");

    // So does a click on the grid's empty space, beside the tiles.
    await tile.click();
    await expect(look).toHaveCount(0);
    await page.locator(".gallery-scroll").click({ position: { x: 4, y: 4 } });
    await expect(look).toBeVisible();

    // And the way back under the preview, which switches to Windows' folder from there.
    await tile.click();
    await page.getByRole("button", { name: "Back to the empty folder" }).click();
    await look.getByRole("radio", { name: "Windows" }).click();
    await expect(look.getByRole("radio", { name: "Windows" })).toBeChecked();

    // Escape while typing a search empties the search, and the skin stays.
    await tile.click();
    await page.getByLabel("search skins").fill("mona");
    await page.getByLabel("search skins").press("Escape");
    await expect(waiting).toBeVisible();
  });
});
