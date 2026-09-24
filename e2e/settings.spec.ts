import { expect, test, type Page } from "@playwright/test";
import { openApp } from "./app";

const dialog = (page: Page) => page.getByRole("dialog", { name: "Settings" });

async function openSettings(page: Page) {
  await page.getByRole("button", { name: /^settings$/i }).first().click();
  await expect(dialog(page)).toBeVisible();
}

const html = (page: Page, attr: string) => page.evaluate((a) => document.documentElement.getAttribute(a), attr);

test.describe("settings", () => {
  test("is its pages down the side and the chosen one beside them, moved through with the arrow keys", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    const tabs = dialog(page).getByRole("tab");
    await expect(tabs).toHaveText(["General", "AI", "Sharing", "About"]);
    await expect(dialog(page).getByRole("tab", { name: "General" })).toHaveAttribute("aria-selected", "true");
    await expect(dialog(page).getByRole("tab", { name: "General" })).toBeFocused();
    await expect(dialog(page).getByRole("heading", { name: "Appearance" })).toBeVisible();
    await page.keyboard.press("ArrowDown");
    await expect(dialog(page).getByRole("tab", { name: "AI" })).toHaveAttribute("aria-selected", "true");
    await expect(dialog(page).getByRole("heading", { name: "Where pictures are made" })).toBeVisible();
    await page.keyboard.press("End");
    await expect(dialog(page).getByRole("heading", { name: "Updates" })).toBeVisible();
    // Nothing in it uses the system's tooltips.
    const titled = await dialog(page).evaluate((d) => [...d.querySelectorAll("[title], svg title")].length);
    expect(titled).toBe(0);
  });

  test("theme, accent colour and motion change the whole app, and are kept", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    await dialog(page).getByRole("radiogroup", { name: "theme" }).getByRole("radio", { name: "Dark" }).click();
    await expect.poll(() => html(page, "data-theme")).toBe("dark");
    await dialog(page).getByRole("radio", { name: "Purple" }).click();
    await expect.poll(() => html(page, "data-accent")).toBe("purple");
    // The accent is the app's: the primary buttons take it.
    const accent = await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--accent").trim());
    expect(accent).toBe("#8b5cf6");
    await dialog(page).getByRole("radiogroup", { name: "motion" }).getByRole("radio", { name: "Reduced" }).click();
    await expect.poll(() => html(page, "data-motion")).toBe("reduced");

    await page.reload();
    await expect(page.getByRole("button", { name: /all skins/i }).first()).toBeVisible();
    expect(await html(page, "data-theme")).toBe("dark");
    expect(await html(page, "data-accent")).toBe("purple");
    expect(await html(page, "data-motion")).toBe("reduced");

    // Black and white is black on the light window and white on the dark one, with dark words on it.
    await openSettings(page);
    await dialog(page).getByRole("radio", { name: "Black and white" }).click();
    const mono = () =>
      page.evaluate(() => {
        const css = getComputedStyle(document.documentElement);
        return [css.getPropertyValue("--accent").trim(), css.getPropertyValue("--on-accent").trim()];
      });
    expect(await mono()).toEqual(["#f2eeed", "#1f1c1c"]);
    await dialog(page).getByRole("radiogroup", { name: "theme" }).getByRole("radio", { name: "Light" }).click();
    await expect.poll(mono).toEqual(["#1d1d1f", "#ffffff"]);

    // Back to the defaults: no attributes left behind.
    await dialog(page).getByRole("radio", { name: "Blue" }).click();
    await dialog(page).getByRole("radiogroup", { name: "motion" }).getByRole("radio", { name: "System" }).click();
    await expect.poll(() => html(page, "data-accent")).toBeNull();
    await expect.poll(() => html(page, "data-motion")).toBeNull();
  });

  test("Motion: Reduced keeps the icons and the library's folders still", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    // The globe on the Sharing tab draws itself again when pointed at, unless motion is reduced.
    const globe = dialog(page).getByRole("tab", { name: "Sharing" }).locator("svg");
    const drawn = async () => {
      // Its way back from being drawn takes a little over a second.
      await page.mouse.move(0, 0);
      await page.waitForTimeout(1500);
      const before = await globe.evaluate((s) => s.outerHTML);
      await dialog(page).getByRole("tab", { name: "Sharing" }).hover();
      await page.waitForTimeout(250);
      return before !== (await globe.evaluate((s) => s.outerHTML));
    };
    expect(await drawn()).toBe(true);
    await dialog(page).getByRole("radiogroup", { name: "motion" }).getByRole("radio", { name: "Reduced" }).click();
    expect(await drawn()).toBe(false);

    await page.keyboard.press("Escape");
    await expect(dialog(page)).toBeHidden();
    const tile = page.locator(".tile-hit").first();
    const box = (await tile.boundingBox())!;
    await page.mouse.move(box.x + box.width * 0.85, box.y + box.height * 0.2);
    await page.mouse.move(box.x + box.width * 0.9, box.y + box.height * 0.15);
    await page.waitForTimeout(300);
    expect(await tile.locator(".tile-art").evaluate((el) => (el as HTMLElement).style.transform)).toBe("");
  });

  test("the accent colours are chosen with the arrow keys too", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    await dialog(page).getByRole("radio", { name: "Blue" }).focus();
    await page.keyboard.press("ArrowRight");
    await expect(dialog(page).getByRole("radio", { name: "Purple" })).toHaveAttribute("aria-checked", "true");
    await expect(dialog(page).getByRole("radio", { name: "Purple" })).toBeFocused();
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowLeft");
    await expect(dialog(page).getByRole("radio", { name: "Black and white" })).toHaveAttribute("aria-checked", "true");
  });

  test("folds the sidebar to its icons and back", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    const sidebar = dialog(page).getByRole("radiogroup", { name: "sidebar" });
    await sidebar.getByRole("radio", { name: "Icons only" }).click();
    await expect(page.locator(".app")).toHaveClass(/is-rail/);
    await sidebar.getByRole("radio", { name: "Full" }).click();
    await expect(page.locator(".app")).not.toHaveClass(/is-rail/);
  });

  test("search keeps the pages that have it and lights up the setting", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    const search = dialog(page).getByLabel("search settings");
    await search.fill("licence");
    await expect(dialog(page).getByRole("tab")).toHaveText(["Sharing"]);
    await expect(dialog(page).getByRole("heading", { name: "Licence profiles" })).toBeVisible();
    await search.fill("accent");
    await expect(dialog(page).getByRole("tab")).toHaveText(["General"]);
    await expect(dialog(page).locator(".set-row.is-match")).toHaveText(/Accent colour/);
    await search.fill("zzzz");
    await expect(dialog(page).getByText(/No setting matches/)).toBeVisible();
    await search.fill("");
    await expect(dialog(page).getByRole("tab")).toHaveCount(4);
  });

  test("licence profiles are added, made the default, checked, deleted and brought back", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    await dialog(page).getByRole("tab", { name: "Sharing" }).click();
    const rows = dialog(page).locator(".set-profile");
    await expect(rows).toHaveCount(1);
    await expect(rows.first()).toContainText("Personal");
    await expect(rows.first()).toContainText("Default");

    await dialog(page).getByRole("button", { name: "Add a profile" }).click();
    const form = dialog(page).getByRole("form", { name: "new profile" });
    await form.getByLabel("Name").fill("personal");
    await form.getByRole("button", { name: "Add profile" }).click();
    await expect(form.getByRole("alert")).toHaveText(/already/);
    await form.getByLabel("Name").fill("For work");
    await form.getByLabel("Credited to").fill("acme studio");
    await form.getByRole("button", { name: "Add profile" }).click();
    await expect(form.getByRole("alert")).toHaveText(/GitHub user name/);
    await form.getByLabel("Credited to").fill("acme-studio");
    await form.getByRole("button", { name: "licence" }).click();
    await page.getByRole("option", { name: /CC BY 4.0/ }).click();
    await form.getByRole("button", { name: "Add profile" }).click();
    await expect(rows).toHaveCount(2);
    await expect(rows.nth(1)).toContainText("Credited to acme-studio · CC BY 4.0");

    await rows.nth(1).hover();
    await rows.nth(1).getByRole("button", { name: "make For work the default" }).click();
    await expect(rows.nth(1)).toContainText("Default");
    await expect(rows.first()).not.toContainText("Default");

    await rows.first().hover();
    await rows.first().getByRole("button", { name: "delete Personal" }).click();
    await expect(rows).toHaveCount(1);
    await dialog(page).getByRole("button", { name: "Undo" }).click();
    await expect(rows).toHaveCount(2);

    // Kept for next time.
    await page.reload();
    await expect(page.getByRole("button", { name: /all skins/i }).first()).toBeVisible();
    await openSettings(page);
    await dialog(page).getByRole("tab", { name: "Sharing" }).click();
    await expect(rows).toHaveCount(2);
    await expect(rows.nth(1)).toContainText("Default");
  });
});
