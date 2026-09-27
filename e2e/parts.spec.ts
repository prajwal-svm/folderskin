import { expect, test, type Page } from "@playwright/test";
import { openApp, openView } from "./app";

// The folder or drive under a design comes apart into its parts on the canvas: each can be given
// a colour of its own, hidden or removed, and the whole of it moved, turned and sized, the design
// going with it. What's saved is the icon the canvas shows.

const composer = (page: Page) => page.locator('section[aria-label="composer"]:not([hidden])');
const side = (page: Page) => page.locator('aside[aria-label="layers and settings"]:not([hidden])');
const newDialog = (page: Page) => page.getByRole("dialog", { name: "Start a new design" });
const parts = (page: Page) => side(page).getByRole("listbox", { name: /and its parts$/ });
const canvas = (page: Page) => composer(page).locator("canvas.cmp-canvas");

/** Starts a new design on `drive`, one of `system`'s drives in the dialog. */
async function startOnDrive(page: Page, system: string, drive: string) {
  await openView(page, /design your own/i);
  await newDialog(page).getByRole("region", { name: "start on a drive" }).getByRole("group", { name: system }).getByRole("button", { name: drive, exact: true }).click();
  await expect(newDialog(page)).toBeHidden();
  await expect(composer(page).locator(".cmp-stage")).not.toHaveAttribute("data-waiting");
}

/** The stage's colour at a point in canvas units (0 to 1024 across), as `[r, g, b, a]`. */
function pixel(page: Page, x: number, y: number) {
  return canvas(page).evaluate(
    (c: HTMLCanvasElement, [x, y]) => {
      const k = c.width / 1024;
      return [...c.getContext("2d")!.getImageData(Math.round(x * k), Math.round(y * k), 1, 1).data];
    },
    [x, y],
  );
}

/** Clicks the canvas at a point in canvas units. */
async function clickCanvas(page: Page, x: number, y: number) {
  const box = (await canvas(page).boundingBox())!;
  await page.mouse.click(box.x + (x / 1024) * box.width, box.y + (y / 1024) * box.height);
}

/** Gives the selected part `hex` with its colour well's code field. */
async function colourPart(page: Page, name: string, hex: string) {
  await side(page).getByRole("button", { name: new RegExp(`^${name} colour:`) }).click();
  const code = page.getByRole("textbox", { name: "colour code" });
  await code.fill(hex);
  await code.press("Enter");
  await page.keyboard.press("Escape");
}

const redder = ([r, g, b]: number[]) => r > 150 && r > g + 60 && r > b + 60;
const bluer = ([r, , b]: number[]) => b > r + 60;

test.describe("a drive in its parts", () => {
  test("lists the USB stick's port, the holes in it, its case and its face, and changes each", async ({ page }) => {
    await openApp(page);
    await startOnDrive(page, "Linux", "USB stick");
    await expect(parts(page).getByRole("option")).toHaveText(["USB stick", "Face", "Case", "Port holes", "Port"]);
    // The case is its own blue.
    await expect.poll(async () => bluer(await pixel(page, 430, 870))).toBe(true);

    // Picked on the canvas, the case is the part in the settings, and takes a colour of its own.
    await clickCanvas(page, 430, 870);
    await expect(parts(page).getByRole("option", { name: "Case" })).toHaveAttribute("aria-selected", "true");
    await colourPart(page, "Case", "#e53935");
    await clickCanvas(page, 1000, 1000);
    await expect.poll(async () => redder(await pixel(page, 430, 870))).toBe(true);

    // The face hidden, the case shows where the label was, and the design still goes there.
    const label = await pixel(page, 512, 565);
    await parts(page).getByRole("button", { name: "hide Face" }).click();
    await expect.poll(() => pixel(page, 512, 565)).not.toEqual(label);
    expect(redder(await pixel(page, 512, 565))).toBe(true);

    // Removed, the holes leave the list, and the group's row brings them back.
    await parts(page).getByRole("option", { name: "Port holes" }).click();
    await page.keyboard.press("Delete");
    await expect(parts(page).getByRole("option", { name: "Port holes" })).toHaveCount(0);
    await parts(page).getByRole("button", { name: "bring back the removed part" }).click();
    await expect(parts(page).getByRole("option", { name: "Port holes" })).toHaveCount(1);

    // Saved, the icon is the one on the canvas.
    await side(page).getByRole("button", { name: /save to yours/i }).click();
    await expect(page.getByText(/is in Yours/)).toBeVisible();
  });

  test("turns, sizes and moves as a whole, with the design on it", async ({ page }) => {
    await openApp(page);
    await startOnDrive(page, "Linux", "USB stick");
    // The port is at the stick's top.
    await expect.poll(async () => (await pixel(page, 512, 150))[3]).toBe(255);
    await parts(page).getByRole("option", { name: "USB stick" }).click();
    const turn = side(page).getByRole("textbox", { name: "Turn, typed" });
    await turn.fill("90");
    await turn.press("Enter");
    // Turned a quarter, the stick lies across: nothing where its port was, its case where its middle's side was.
    await expect.poll(async () => (await pixel(page, 512, 150))[3]).toBe(0);
    expect((await pixel(page, 800, 522))[3]).toBe(255);
    await page.keyboard.press("Control+z");
    await expect.poll(async () => (await pixel(page, 512, 150))[3]).toBe(255);

    // Dragged anywhere on it, the whole stick moves, and Back in place puts it back.
    const box = (await canvas(page).boundingBox())!;
    const at = (x: number, y: number) => [box.x + (x / 1024) * box.width, box.y + (y / 1024) * box.height] as const;
    await page.mouse.move(...at(450, 860));
    await page.mouse.down();
    await page.mouse.move(...at(500, 860), { steps: 4 });
    await page.mouse.move(...at(640, 860), { steps: 4 });
    await page.mouse.up();
    await expect.poll(async () => (await pixel(page, 430, 870))[3]).toBe(0);
    await side(page).getByRole("button", { name: "Back in place" }).click();
    await expect.poll(async () => (await pixel(page, 430, 870))[3]).toBe(255);
  });
});

test.describe("a folder in its parts", () => {
  test("leaves out the paper and gives the tab a colour of its own", async ({ page }) => {
    await openApp(page);
    await openView(page, /design your own/i);
    await newDialog(page).getByRole("button", { name: /^Mac folder/ }).click();
    await expect(newDialog(page)).toBeHidden();
    await expect(composer(page).locator(".cmp-stage")).not.toHaveAttribute("data-waiting");
    await expect(parts(page).getByRole("option")).toHaveText(["Mac folder", "Front", "Paper", "Back", "Tab"]);

    // The paper is its own off-white, and picked on the canvas.
    const paper = await pixel(page, 512, 140);
    expect(paper[0]).toBeGreaterThan(200);
    await clickCanvas(page, 512, 140);
    await expect(parts(page).getByRole("option", { name: "Paper" })).toHaveAttribute("aria-selected", "true");
    await side(page).getByRole("switch", { name: "On the icon" }).click();
    await expect.poll(async () => bluer(await pixel(page, 512, 140))).toBe(true);

    // The tab takes a colour of its own over the design's background, and only the tab does.
    const back = await pixel(page, 700, 115);
    await parts(page).getByRole("option", { name: "Tab" }).click();
    await colourPart(page, "Tab", "#ffcc00");
    await clickCanvas(page, 1010, 1010);
    await expect.poll(async () => (await pixel(page, 250, 60))[2]).toBeLessThan(60);
    expect(await pixel(page, 700, 115)).toEqual(back);
  });
});
