import { expect, test, type Page } from "@playwright/test";
import { letGo, openApp, openView } from "./app";

const chat = (page: Page) => page.locator('section[aria-label="generate with AI"]:not([hidden])');
const box = (page: Page) => chat(page).getByLabel("describe the folder");
const settings = (page: Page) => page.getByRole("dialog", { name: "Where pictures are made" });
/** The shape chip in the prompt's bar, named by the shape it's on. */
const shapeChip = (page: Page) => chat(page).locator(".shape-chip");
/** The "@" or "/" menu over the prompt. */
const menu = (page: Page) => page.locator(".pm");
const option = (page: Page, name: string | RegExp) => menu(page).getByRole("option", { name });
const app = (page: Page) => page.locator("main.app");

/** Opens the AI view with an OpenAI key saved (the preview keeps keys for the page's life). */
async function withKey(page: Page, query = "") {
  await openApp(page, { query });
  await openView(page, /generate with ai/i);
  await chat(page).locator(".model-pill").click();
  await settings(page).getByRole("radio", { name: /OpenAI/ }).click();
  await settings(page).getByPlaceholder(/Paste your key/).fill("sk-test");
  await settings(page).getByRole("button", { name: /Save and check/ }).click();
  await expect(settings(page).getByText(/Saved on this device|Saved in this browser preview/)).toBeVisible();
  await settings(page).getByRole("button", { name: "close" }).click();
}

test.describe("the shape a picture is for", () => {
  test("is picked from its chip, or with @ in the box, by mouse or keys", async ({ page }) => {
    await openApp(page);
    await openView(page, /generate with ai/i);
    // The folder the app puts skins on, until another is picked.
    await expect(shapeChip(page)).toHaveAccessibleName("Mac folder: choose what it's for");

    await shapeChip(page).click();
    const shapes = page.getByRole("dialog", { name: "shapes" });
    await expect(shapes.getByRole("radio", { name: /Mac folder/ })).toHaveAttribute("aria-checked", "true");
    await shapes.getByRole("radio", { name: /Windows folder/ }).click();
    await expect(shapes).toHaveCount(0);
    await expect(shapeChip(page)).toHaveAccessibleName("Windows folder: choose what it's for");

    // "@" narrows as it's typed; Enter takes the first, and what was typed to open it goes.
    await box(page).click();
    await box(page).pressSequentially("a cheerful fox @fr");
    await expect(menu(page).getByRole("option")).toHaveCount(1);
    await expect(option(page, /Free icon/)).toHaveAttribute("aria-selected", "true");
    await box(page).press("Enter");
    await expect(menu(page)).toHaveCount(0);
    await expect(box(page)).toHaveValue("a cheerful fox ");
    await expect(shapeChip(page)).toHaveAccessibleName("Free icon: choose what it's for");
    // A free icon asks for an icon, and has no whole-or-art choice.
    await box(page).fill("");
    await expect(box(page)).toHaveAttribute("placeholder", /Describe the icon you want/);
    await shapeChip(page).click();
    await expect(page.getByRole("dialog", { name: "shapes" }).getByRole("radiogroup", { name: /What to make/i })).toHaveCount(0);
    await page.keyboard.press("Escape");

    // The arrow keys move through the rows, Tab chooses too, and Escape puts the menu away.
    await box(page).pressSequentially("@");
    await expect(menu(page).getByRole("option")).toHaveCount(3);
    await box(page).press("ArrowDown");
    await expect(option(page, /Windows folder/)).toHaveAttribute("aria-selected", "true");
    await box(page).press("Tab");
    await expect(shapeChip(page)).toHaveAccessibleName("Windows folder: choose what it's for");
    await box(page).pressSequentially("@zz");
    await expect(menu(page)).toContainText("No shape is called anything like “zz”.");
    await box(page).press("Escape");
    await expect(menu(page)).toHaveCount(0);
    await expect(box(page)).toHaveValue("@zz");

    // And with the mouse.
    await box(page).fill("");
    await box(page).pressSequentially("@");
    await option(page, /Mac folder/).click();
    await expect(shapeChip(page)).toHaveAccessibleName("Mac folder: choose what it's for");
    await expect(box(page)).toHaveValue("");
  });

  test("is part of the chat: a picture is made for it, and an older chat opens on its own", async ({ page }) => {
    await withKey(page);
    await box(page).pressSequentially("@free");
    await box(page).press("Enter");
    await box(page).fill("a cheerful fox mascot");
    await box(page).press("Enter");
    const card = chat(page).locator("article.turn").last();
    await expect(card.locator(".turn-for")).toHaveText("Free icon");
    await expect(card.getByRole("button", { name: "Choose a folder" })).toBeVisible({ timeout: 10_000 });

    await chat(page).getByRole("button", { name: "new chat" }).click();
    await expect(shapeChip(page)).toHaveAccessibleName("Mac folder: choose what it's for");
    await chat(page).getByRole("button", { name: "chats", exact: true }).click();
    await page.getByRole("complementary", { name: "chats" }).locator(".chat-open", { hasText: "A cheerful fox" }).click();
    await expect(shapeChip(page)).toHaveAccessibleName("Free icon: choose what it's for");
  });
});

test.describe("the / menu", () => {
  test("lists the styles under their headings, and a style picked goes in its own slot", async ({ page }) => {
    await withKey(page);
    await box(page).click();
    await box(page).pressSequentially("/");
    for (const heading of ["Photo and 3D", "Materials and craft", "Painting and drawing", "Print", "Digital and graphic", "Ideas"]) {
      await expect(menu(page).getByRole("group", { name: heading })).toBeAttached();
    }
    await box(page).pressSequentially("wood");
    await expect(option(page, /Woodblock print/)).toHaveAttribute("aria-selected", "true");
    await box(page).press("Enter");
    await expect(box(page)).toHaveValue("");
    const look = chat(page).locator(".composer-style");
    await expect(look).toContainText("Woodblock print");
    // The chip under the box for the same style reads as the one picked.
    await expect(chat(page).locator(".style-chip.is-active")).toHaveText("Woodblock print");

    await box(page).fill("a koi pond at night");
    await box(page).press("Enter");
    const card = chat(page).locator("article.turn").last();
    await expect(card.locator(".turn-for")).toHaveText("Mac folder · Woodblock print");
    await expect(card.locator(".tag-chip", { hasText: "woodblock" })).toBeVisible({ timeout: 10_000 });

    // Taken off, the next picture has none.
    await chat(page).getByRole("button", { name: "remove the style Woodblock print" }).click();
    await expect(look).toHaveCount(0);
  });

  test("an idea to start from fills the box, and a style chip puts its style in the slot", async ({ page }) => {
    await openApp(page);
    await openView(page, /generate with ai/i);
    await chat(page).getByRole("button", { name: "Travel poster" }).click();
    await expect(box(page)).toHaveValue(/tiny red seaplane/);
    await expect(chat(page).locator(".composer-style")).toContainText("Travel poster");
    await box(page).fill("");
    await box(page).pressSequentially("/light");
    await option(page, /Lighthouse/).click();
    await expect(box(page)).toHaveValue(/a lighthouse on a rocky point/);
  });

  test("saves what's in the box as a prompt, with its style, and brings both back", async ({ page }) => {
    await withKey(page);
    await box(page).pressSequentially("/woodblock");
    await box(page).press("Enter");
    await box(page).fill("a koi pond at night");
    await box(page).pressSequentially(" /");
    // With words in the box, saving them comes first.
    await expect(option(page, /Save as a prompt/)).toHaveAttribute("aria-selected", "true");
    await box(page).press("Enter");
    const name = menu(page).getByLabel("prompt name");
    await expect(name).toBeFocused();
    await expect(name).toHaveValue("a koi pond at night");
    await expect(menu(page)).toContainText("Saved with Woodblock print");
    await name.fill("Moody koi");
    await name.press("Enter");
    await expect(page.getByText("Saved “Moody koi” to Your prompts")).toBeVisible();
    await expect(box(page)).toHaveValue("a koi pond at night ");

    // Back from Your prompts, words and style together.
    await box(page).fill("");
    await chat(page).getByRole("button", { name: "remove the style Woodblock print" }).click();
    await box(page).pressSequentially("/moody");
    await expect(option(page, /Moody koi/)).toContainText("Woodblock print · a koi pond at night");
    await option(page, /Moody koi/).click();
    await expect(box(page)).toHaveValue("a koi pond at night");
    await expect(chat(page).locator(".composer-style")).toContainText("Woodblock print");

    // The same name, whatever its capitals, replaces it.
    await box(page).pressSequentially(" /");
    await box(page).press("Enter");
    await menu(page).getByLabel("prompt name").fill("MOODY KOI");
    await expect(menu(page)).toContainText("“Moody koi” is saved already. Saving replaces it.");
    await menu(page).getByRole("button", { name: "Replace" }).click();
    await expect(page.getByText("Replaced “MOODY KOI” in Your prompts")).toBeVisible();

    // Kept with the app's data: there after a restart.
    await page.reload();
    await openView(page, /generate with ai/i);
    await box(page).click();
    await box(page).pressSequentially("/");
    await expect(menu(page).getByRole("group", { name: "Your prompts" }).getByRole("option")).toHaveCount(1);

    // Removed, with the chance to put it back.
    await option(page, /MOODY KOI/).hover();
    await menu(page).getByRole("button", { name: "remove MOODY KOI" }).click();
    await expect(menu(page).getByRole("group", { name: "Your prompts" })).toHaveCount(0);
    await page.getByRole("button", { name: "Undo" }).click();
    await box(page).click();
    await expect(menu(page).getByRole("group", { name: "Your prompts" }).getByRole("option", { name: /MOODY KOI/ })).toBeVisible();
  });

  test("a prompt that names someone as its style is saved after a word", async ({ page }) => {
    await openApp(page);
    await openView(page, /generate with ai/i);
    await box(page).fill("a forest spirit in the style of Studio Ghibli");
    await box(page).pressSequentially(" /");
    await box(page).press("Enter");
    await expect(menu(page)).toContainText("It names Studio Ghibli as its style. Describing the technique works better.");
    await expect(menu(page).getByRole("button", { name: "Save anyway" })).toBeVisible();
    await menu(page).getByLabel("prompt name").press("Escape");
    await expect(menu(page).getByLabel("prompt name")).toHaveCount(0);
    await expect(box(page)).toBeFocused();
  });
});

test.describe("a reference picture", () => {
  test("is used for its subject unless it's made the style or the colours", async ({ page }) => {
    await withKey(page);
    await chat(page).getByRole("button", { name: "add a reference picture" }).click();
    const picture = chat(page).getByRole("button", { name: "Reference.jpg: Subject" });
    await picture.click();
    const roles = page.getByRole("dialog", { name: "what the picture is for" });
    await expect(roles.getByRole("radio", { name: /Subject/ })).toHaveAttribute("aria-checked", "true");
    await roles.getByRole("radio", { name: /Style/ }).click();
    await expect(chat(page).getByRole("button", { name: "Reference.jpg: Style" })).toContainText("Style");
    await box(page).fill("a lighthouse at dusk");
    await box(page).press("Enter");
    await expect(chat(page).locator("article.turn .turn-refs img").first()).toHaveAttribute("data-tip", "Reference.jpg · Style");
  });
});

test.describe("the chat", () => {
  test("stays as it was while other views are on show, with a picture still being made, until the app starts again", async ({ page }) => {
    await withKey(page, "holdpaint");
    await box(page).fill("a paper boat");
    await box(page).press("Enter");
    const card = chat(page).locator("article.turn").last();
    await expect(card.getByRole("button", { name: "Stop" })).toBeVisible();
    // The next idea on its way, with its style, its shape and a picture.
    await box(page).fill("a lighthouse at dusk");
    await box(page).pressSequentially(" /clay");
    await box(page).press("Enter");
    await box(page).pressSequentially("@win");
    await box(page).press("Enter");
    await chat(page).getByRole("button", { name: "add a reference picture" }).click();
    await expect(chat(page).getByRole("button", { name: "remove Reference.jpg" })).toBeVisible();

    for (const view of [/all skins/i, /design your own/i, /community/i]) {
      await openView(page, view);
      await expect(chat(page)).toHaveCount(0);
      await openView(page, /generate with ai/i);
      await expect(card.getByRole("button", { name: "Stop" })).toBeVisible();
      await expect(box(page)).toHaveValue("a lighthouse at dusk ");
      await expect(chat(page).locator(".composer-style")).toContainText("Clay");
      await expect(shapeChip(page)).toHaveAccessibleName("Windows folder: choose what it's for");
      await expect(chat(page).getByRole("button", { name: "remove Reference.jpg" })).toBeVisible();
    }
    // Settings over it leaves it too.
    await page.getByRole("button", { name: /^settings$/i }).first().click();
    await page.keyboard.press("Escape");
    await expect(box(page)).toHaveValue("a lighthouse at dusk ");

    // The picture lands in its card while the chat is out of sight, and is there on coming back.
    await openView(page, /all skins/i);
    await letGo(page, "mockPaintGo");
    await openView(page, /generate with ai/i);
    await expect(card.getByRole("button", { name: "Choose a folder" })).toBeVisible({ timeout: 10_000 });

    // A new launch starts a fresh chat, with this one in the history.
    await page.reload();
    await openView(page, /generate with ai/i);
    await expect(chat(page).getByRole("heading", { name: /what should your folder look like/i })).toBeVisible();
    await expect(box(page)).toHaveValue("");
    await chat(page).getByRole("button", { name: "chats", exact: true }).click();
    await expect(page.getByRole("complementary", { name: "chats" }).locator(".chat-open", { hasText: "A paper boat" })).toBeVisible();
  });
});

test.describe("the folder panel", () => {
  test("closes and opens again from its button or keys, and stays as it was left", async ({ page }) => {
    await openApp(page);
    await expect(app(page)).not.toHaveClass(/is-right-off/);
    await page.getByRole("button", { name: "close the folder panel" }).click();
    await expect(app(page)).toHaveClass(/is-right-off/);
    await expect(page.locator(".right-slot")).toHaveAttribute("aria-hidden", "true");
    // Remembered, like the folded sidebar.
    await page.reload();
    await expect(page.getByRole("button", { name: "open the folder panel" })).toBeVisible();
    await expect(app(page)).toHaveClass(/is-right-off/);
    // Closed in one view, closed in the others.
    await openView(page, /community/i);
    await expect(app(page)).toHaveClass(/is-right-off/);
    await page.keyboard.press("ControlOrMeta+Shift+Backslash");
    await expect(app(page)).not.toHaveClass(/is-right-off/);
    await page.keyboard.press("ControlOrMeta+Shift+Backslash");
    await expect(app(page)).toHaveClass(/is-right-off/);
    // The composer's own panel is always there, with no button to close it.
    await openView(page, /design your own/i);
    await expect(app(page)).not.toHaveClass(/is-right-off/);
    await expect(page.locator("button.panel-toggle")).toHaveCount(0);
  });

  test("in the AI chat, has a button once there's a folder, and a folder chosen opens it", async ({ page }) => {
    await openApp(page);
    await page.getByRole("button", { name: "close the folder panel" }).click();
    await openView(page, /generate with ai/i);
    await expect(page.locator("button.panel-toggle")).toHaveCount(0);
    await chat(page).getByRole("button", { name: "Choose a folder" }).click();
    await expect(app(page)).not.toHaveClass(/is-right-off/);
    await page.getByRole("button", { name: "close the folder panel" }).click();
    await expect(app(page)).toHaveClass(/is-right-off/);
    await page.getByRole("button", { name: "open the folder panel" }).click();
    await expect(app(page)).not.toHaveClass(/is-right-off/);
  });
});
