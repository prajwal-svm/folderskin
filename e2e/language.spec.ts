import { expect, test, type Page } from "@playwright/test";
import { openApp } from "./app";

// The sidebar and the row by their classes, not their names: a name is in the language on show
// ("secciones", "Langue : Français").
const sidebar = (page: Page) => page.locator("nav.sidebar");
const row = (page: Page) => sidebar(page).locator("button.nav-lang");
const menu = (page: Page) => page.getByRole("menu", { name: "Language" });
const saved = (page: Page) => page.evaluate(() => JSON.parse(localStorage.getItem("folderskin.prefs") ?? "{}").language ?? null);

/**
 * Japanese with one word in it, so a test can see the app draw again in the language chosen
 * whatever the catalogs hold. The dev server serves each catalog as a module of its own.
 */
async function someJapanese(page: Page) {
  await page.route("**/src/locales/ja/sidebar.json*", (route) =>
    route.fulfill({ contentType: "text/javascript", body: `export default ${JSON.stringify({ items: { skins: "すべてのスキン" } })};` }),
  );
}

test.describe("the language menu", () => {
  test("sits under Dark mode and over Settings, and says the language in its own words", async ({ page }) => {
    await openApp(page);
    const buttons = sidebar(page).locator(".sidebar-bottom > button");
    const names = await buttons.evaluateAll((els) => els.map((el) => el.getAttribute("aria-label") ?? el.textContent?.trim()));
    const dark = names.indexOf("dark mode");
    expect(names[dark + 1]).toBe("Language: English");
    expect(names[dark + 2]).toBe("Settings");
    await expect(row(page)).toHaveText("English");
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
  });

  test("lists every language in its own words, and ticks the one on show", async ({ page }) => {
    await openApp(page);
    await row(page).click();
    await expect(menu(page).getByRole("menuitemradio")).toHaveText(["English", "简体中文", "日本語", "한국어", "Français", "Español"]);
    await expect(menu(page).getByRole("menuitemradio", { checked: true })).toHaveText("English");
  });

  test("puts the app in the language chosen, draws it again, and keeps it for the next launch", async ({ page }) => {
    await someJapanese(page);
    await openApp(page);
    await row(page).click();
    await menu(page).getByRole("menuitemradio", { name: "日本語" }).click();
    await expect(menu(page)).toBeHidden();
    await expect(page.locator("html")).toHaveAttribute("lang", "ja");
    await expect(row(page)).toHaveText("日本語");
    await expect(row(page)).toBeFocused();
    await expect(sidebar(page).getByRole("button", { name: /すべてのスキン/ })).toBeVisible();
    // Anything Japanese doesn't have yet is in English.
    await expect(sidebar(page).getByRole("button", { name: "Settings" })).toBeVisible();
    await expect.poll(() => saved(page)).toBe("ja");

    await page.reload();
    await expect(page.locator("html")).toHaveAttribute("lang", "ja");
    await expect(sidebar(page).getByRole("button", { name: /すべてのスキン/ })).toBeVisible();

    await row(page).click();
    await menu(page).getByRole("menuitemradio", { name: "English" }).click();
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
    await expect(sidebar(page).getByRole("button", { name: /all skins/i })).toBeVisible();
    await expect.poll(() => saved(page)).toBe("en");
  });

  test("works from the keyboard", async ({ page }) => {
    await openApp(page);
    await row(page).focus();
    await page.keyboard.press("Enter");
    // The language on show has the focus first.
    await expect(menu(page).getByRole("menuitemradio", { name: "English" })).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(menu(page).getByRole("menuitemradio", { name: "Español" })).toBeFocused();
    await page.keyboard.press("Home");
    await page.keyboard.press("ArrowDown");
    await expect(menu(page).getByRole("menuitemradio", { name: "简体中文" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(menu(page)).toBeHidden();
    await expect(row(page)).toBeFocused();
    await expect(page.locator("html")).toHaveAttribute("lang", "en");

    await page.keyboard.press("ArrowDown");
    await expect(menu(page)).toBeVisible();
    await page.keyboard.press("End");
    await page.keyboard.press("ArrowUp");
    await expect(menu(page).getByRole("menuitemradio", { name: "Français" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.locator("html")).toHaveAttribute("lang", "fr");
    await expect(row(page)).toBeFocused();
    await expect(row(page)).toHaveText("Français");
  });

  test("is the icon alone on the folded sidebar, named in its tooltip, with the menu beside it", async ({ page }) => {
    await openApp(page);
    await sidebar(page).getByRole("button", { name: "collapse the sidebar" }).click();
    await expect(sidebar(page)).toHaveClass(/is-rail/);
    await expect(row(page)).toHaveText("");
    await row(page).hover();
    await expect(page.getByRole("tooltip")).toHaveText("Language: English");
    await row(page).click();
    const box = (await row(page).boundingBox())!;
    const pop = (await menu(page).boundingBox())!;
    expect(pop.x).toBeGreaterThan(box.x + box.width);
    await menu(page).getByRole("menuitemradio", { name: "Español" }).click();
    await expect(page.locator("html")).toHaveAttribute("lang", "es");
    await row(page).hover();
    await expect(page.getByRole("tooltip")).toHaveText("Idioma: Español");
  });

  test("keeps the focus on the row when a language is picked the moment the pointer reaches it", async ({ page }) => {
    await openApp(page);
    await row(page).click();
    await expect(menu(page)).toBeVisible();
    // The click lands as soon as the pointer's move onto 日本語 is drawn, before anything React
    // runs after drawing. The focus once went back into the closing menu then, and was lost.
    const focused = await page.evaluate(
      () =>
        new Promise<string>((resolve) => {
          const item = document.querySelector<HTMLElement>('.lang-pop [data-index="2"]')!;
          const watch = new MutationObserver(() => {
            if (!item.classList.contains("is-active")) return;
            watch.disconnect();
            item.click();
            setTimeout(() => resolve(document.activeElement?.className ?? ""), 300);
          });
          watch.observe(item, { attributes: true, attributeFilter: ["class"] });
          item.dispatchEvent(new MouseEvent("mouseover", { bubbles: true, relatedTarget: document.body }));
        }),
    );
    await expect(page.locator("html")).toHaveAttribute("lang", "ja");
    expect(focused).toContain("nav-lang");
  });

  test("closes when something else is pressed, and changes nothing", async ({ page }) => {
    await openApp(page);
    await row(page).click();
    await expect(menu(page)).toBeVisible();
    await page.locator(".stage-island").click({ position: { x: 12, y: 12 } });
    await expect(menu(page)).toBeHidden();
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
  });
});

/** Opens the app without waiting on any word in it, which is in the computer's language here. */
async function openIn(page: Page) {
  await page.addInitScript(() => localStorage.setItem("folderskin.mock.onboarded", "1"));
  await page.goto("/?yours=8");
  await expect(page.locator(".sidebar .nav-lang")).toBeVisible();
}

test.describe("the first launch", () => {
  test.use({ locale: "ko-KR" });

  test("opens in the computer's language when the app speaks it", async ({ page }) => {
    await openIn(page);
    await expect(page.locator("html")).toHaveAttribute("lang", "ko");
    await expect(page.locator(".sidebar .nav-lang")).toHaveText("한국어");
    // Nothing is saved until a language is chosen: the computer's can still change.
    expect(await saved(page)).toBeNull();
  });
});

test.describe("a computer in a language the app doesn't speak", () => {
  test.use({ locale: "de-DE" });

  test("opens in English", async ({ page }) => {
    await openIn(page);
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
    await expect(row(page)).toHaveText("English");
  });
});
