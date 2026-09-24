import { expect, test, type Page } from "@playwright/test";
import { openApp, openView } from "./app";

const chat = (page: Page) => page.locator('section[aria-label="generate with AI"]:not([hidden])');
const settings = (page: Page) => page.getByRole("dialog", { name: "Where pictures are made" });
const box = (page: Page) => chat(page).getByLabel("describe the folder");
const folderPanel = (page: Page) => page.locator(".right-slot");

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

async function sendIdea(page: Page, idea: string) {
  await box(page).fill(idea);
  await box(page).press("Enter");
}

/** Chats saved before the app opens, newest first, as the preview keeps them (devMock's chats). */
async function seedChats(page: Page, titles: string[]) {
  await page.addInitScript((titles) => {
    const key = "folderskin.mock.chats";
    if (localStorage.getItem(key)) return;
    const chats = titles.map((title, i) => {
      const at = Date.now() - (i + 1) * 60_000;
      return {
        version: 1,
        id: `cseed${i}`,
        title,
        named: false,
        created: at,
        updated: at,
        folder: null,
        turns: [{ id: "t1", idea: title.toLowerCase(), shape: "folder", provider: "openai", model: "gpt-image-2.5-flare", where: "OpenAI · GPT Image 2.5 Flare", refs: [], status: "stopped", started: at, finished: at }],
      };
    });
    const index = chats.map((c) => ({ id: c.id, title: c.title, created: c.created, updated: c.updated, turns: 1, cover: null }));
    localStorage.setItem(key, JSON.stringify({ index, chats: Object.fromEntries(chats.map((c) => [c.id, c])) }));
  }, titles);
}

test.describe("the AI chat", () => {
  test("has the window to itself until a folder is chosen, then the folder slides in", async ({ page }) => {
    await openApp(page);
    const centre = page.locator("section.island-main").first();
    const before = (await centre.boundingBox())!.width;
    await openView(page, /generate with ai/i);
    await expect(page.locator("main.app")).toHaveClass(/is-right-off/);
    await expect.poll(async () => (await centre.boundingBox())!.width).toBeGreaterThan(before + 250);
    await chat(page).getByRole("button", { name: "Choose a folder" }).click();
    await expect(page.locator("main.app")).not.toHaveClass(/is-right-off/);
    await expect(folderPanel(page).getByRole("heading", { name: "Projects" })).toBeVisible();
    // Hidden by choice, and back.
    await chat(page).getByRole("button", { name: /for the folder Projects/ }).click();
    await page.getByRole("menuitem", { name: "Hide it on the right" }).click();
    await expect(page.locator("main.app")).toHaveClass(/is-right-off/);
    await chat(page).getByRole("button", { name: /for the folder Projects/ }).click();
    await page.getByRole("menuitem", { name: "Show it on the right" }).click();
    await expect(page.locator("main.app")).not.toHaveClass(/is-right-off/);
    await chat(page).getByRole("button", { name: /for the folder Projects/ }).click();
    await page.getByRole("menuitem", { name: "Don't use a folder" }).click();
    await expect(chat(page).getByRole("button", { name: "Choose a folder" })).toBeVisible();
    await expect(page.locator("main.app")).toHaveClass(/is-right-off/);
    // Everywhere else the folder panel is where it always was.
    await openView(page, /community/i);
    await expect(page.locator("main.app")).not.toHaveClass(/is-right-off/);
  });

  test("shows what's happening while it paints, and the result can be applied from the chat", async ({ page }) => {
    await withKey(page);
    await sendIdea(page, "a lighthouse at dusk, oil painting");
    const card = chat(page).locator("article.turn").last();
    await expect(card.locator(".turn-name")).toHaveText(/Sending your idea to OpenAI|OpenAI is painting it/);
    await expect(card.getByRole("button", { name: "Stop" })).toBeVisible();
    await expect(card.getByRole("button", { name: "Choose a folder…" })).toBeVisible({ timeout: 10_000 });
    // Named after its first words, never ending on a little one.
    await expect(card.locator(".turn-name")).toHaveText("A lighthouse at dusk");
    await card.getByRole("button", { name: "Choose a folder…" }).click();
    await card.getByRole("button", { name: "Preview" }).click();
    await expect(card.getByRole("button", { name: "On show" })).toBeDisabled();
    await card.getByRole("button", { name: "Apply to Projects" }).click();
    await expect(card.getByText("On Projects")).toBeVisible();
  });

  test("keeps its chats: they are there after a restart, and can be found, renamed and deleted", async ({ page }) => {
    await withKey(page);
    await sendIdea(page, "pop art cats");
    await expect(chat(page).locator(".turn-result:not(.is-developing)")).toHaveCount(1, { timeout: 10_000 });
    await chat(page).getByRole("button", { name: "new chat" }).click();
    await expect(chat(page).getByRole("heading", { name: /what should your folder look like/i })).toBeVisible();
    await page.waitForTimeout(500);
    await page.reload();
    await openView(page, /generate with ai/i);
    // The last chat with something in it opens again, with what was asked in it. (The preview's
    // library doesn't outlive a reload, so its card says the picture has gone, as the app does.)
    await expect(chat(page).locator(".studio-chat-title")).toHaveText("Pop art cats");
    await expect(chat(page).locator(".turn-ask")).toContainText("pop art cats");
    await expect(chat(page).getByText("This picture has been deleted from your library.")).toBeVisible();
    await chat(page).getByRole("button", { name: "chats", exact: true }).click();
    const drawer = page.getByRole("complementary", { name: "chats" });
    await drawer.getByLabel("search chats").fill("lighthouse");
    await expect(drawer.getByText(/No chat is called anything like/)).toBeVisible();
    await drawer.getByLabel("search chats").fill("cats");
    await drawer.getByRole("button", { name: "rename Pop art cats" }).click();
    await drawer.getByLabel("chat name").fill("Cats for the desktop");
    await drawer.getByLabel("chat name").press("Enter");
    await expect(chat(page).locator(".studio-chat-title")).toHaveText("Cats for the desktop");
    await drawer.getByRole("button", { name: "delete Cats for the desktop" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Delete" }).click();
    await expect(drawer.getByText(/Your chats will be here|No chat is called/)).toBeVisible();
  });

  test("a request can be stopped", async ({ page }) => {
    await withKey(page);
    await sendIdea(page, "a quiet harbour");
    const card = chat(page).locator("article.turn").last();
    await card.getByRole("button", { name: "Stop" }).click();
    await expect(card.getByText(/Stopped before it finished/)).toBeVisible();
    await expect(card.getByRole("button", { name: "Try again" })).toBeVisible();
  });

  test("offers the right next step for what went wrong", async ({ page }) => {
    await withKey(page, "aifail=refused");
    await sendIdea(page, "something it won't draw");
    const card = chat(page).locator("article.turn").last();
    await expect(card.getByRole("alert")).toContainText("declined that prompt");
    // Trying the same words again can't help; changing them can.
    await expect(card.getByRole("button", { name: "Try again" })).toHaveCount(0);
    await card.getByRole("button", { name: "Reword it" }).click();
    await expect(box(page)).toHaveValue("something it won't draw");
  });

  test("a network failure can simply be tried again", async ({ page }) => {
    await withKey(page, "aifail=network");
    await sendIdea(page, "a mountain lake");
    const card = chat(page).locator("article.turn").last();
    await expect(card.getByRole("alert")).toContainText(/couldn't reach OpenAI/i);
    await expect(card.getByRole("button", { name: "Try again" })).toBeVisible();
    await expect(card.getByRole("button", { name: /Check the key/ })).toHaveCount(0);
  });

  test("sets this computer up in one click, then paints on it with its progress and log", async ({ page }) => {
    await openApp(page, { query: "aifail=memory" });
    await openView(page, /generate with ai/i);
    await chat(page).locator(".model-pill").click();
    await settings(page).getByRole("radio", { name: /This computer/ }).click();
    await expect(settings(page).getByText("NVIDIA GeForce RTX 3050 Ti, 4 GB")).toBeVisible();
    await settings(page).getByRole("button", { name: "Set up this computer" }).click();
    await expect(settings(page).getByText(/Downloading what this computer needs/)).toBeVisible();
    await expect(settings(page).getByText(/Set up and ready/)).toBeVisible({ timeout: 10_000 });
    await settings(page).getByRole("button", { name: "close" }).click();
    await expect(chat(page).locator(".studio-foot")).toContainText("nothing leaves this computer");
    await sendIdea(page, "a paper boat");
    const card = chat(page).locator("article.turn").last();
    await expect(card.locator(".turn-where")).toContainText(/Step \d of 8/, { timeout: 10_000 });
    await card.getByRole("button", { name: "Details" }).click();
    await expect(card.locator(".turn-log-lines")).toContainText("backend: CUDA");
    // Out of memory: what happened, what to do, and a question ready for Claude.
    await expect(card.getByRole("alert")).toContainText("ran out of memory", { timeout: 10_000 });
    await expect(card.locator(".turn-fix li")).toHaveCount(2);
    await expect(card.getByRole("button", { name: "Ask Claude to fix it" })).toBeVisible();
  });

  test("renames a chat from the history that hasn't been opened", async ({ page }) => {
    await seedChats(page, ["A lighthouse", "Older chat"]);
    await openApp(page);
    await openView(page, /generate with ai/i);
    await expect(chat(page).locator(".studio-chat-title")).toHaveText("A lighthouse");
    await chat(page).getByRole("button", { name: "chats", exact: true }).click();
    const drawer = page.getByRole("complementary", { name: "chats" });
    await drawer.getByRole("button", { name: "rename Older chat" }).click();
    await drawer.getByLabel("chat name").fill("Boats for the desktop");
    await drawer.getByLabel("chat name").press("Enter");
    await expect(drawer.getByRole("button", { name: "rename Boats for the desktop" })).toBeVisible();
    // It stays renamed, and is saved so.
    await page.waitForTimeout(500);
    await expect(drawer.getByRole("button", { name: "rename Older chat" })).toHaveCount(0);
    await drawer.locator(".chat-open", { hasText: "Boats for the desktop" }).click();
    await expect(chat(page).locator(".studio-chat-title")).toHaveText("Boats for the desktop");
    await page.reload();
    await openView(page, /generate with ai/i);
    await chat(page).getByRole("button", { name: "chats", exact: true }).click();
    await expect(drawer.getByRole("button", { name: "rename Boats for the desktop" })).toBeVisible();
  });

  test("this computer paints one picture at a time, whichever chat asks", async ({ page }) => {
    await openApp(page, { query: "localready" });
    await openView(page, /generate with ai/i);
    await expect(chat(page).locator(".studio-foot")).toContainText("nothing leaves this computer");
    await sendIdea(page, "a paper boat");
    await expect(chat(page).locator("article.turn").last().getByRole("button", { name: "Stop" })).toBeVisible();
    await chat(page).getByRole("button", { name: "new chat" }).click();
    await box(page).fill("a lighthouse at dusk");
    const generate = chat(page).getByRole("button", { name: "generate" });
    await expect(generate).toBeDisabled();
    await expect(generate).toHaveAttribute("data-tip", "This computer is still painting the last one");
    // Free again once the first one is done.
    await expect(generate).toBeEnabled({ timeout: 10_000 });
  });

  test("a key that doesn't pass its check is still saved, and says so", async ({ page }) => {
    await withKey(page);
    await chat(page).locator(".model-pill").click();
    await expect(settings(page).getByRole("radio", { name: /OpenAI/ }).getByLabel("Key saved")).toBeVisible();
  });
});
