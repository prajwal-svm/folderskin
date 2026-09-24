import { expect, test, type Page } from "@playwright/test";
import { openApp, openView } from "./app";

const chat = (page: Page) => page.locator('section[aria-label="generate with AI"]:not([hidden])');
const settings = (page: Page) => page.getByRole("dialog", { name: "Where pictures are made" });
/** The Local Model's tile, whose check says it's set up and ready. */
const localReady = (page: Page) => settings(page).getByRole("radio", { name: /Local Model/ }).getByRole("img", { name: "Set up" });
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
    const index = chats.map((c) => ({ id: c.id, title: c.title, created: c.created, updated: c.updated, turns: 1, pictures: 0, cover: null }));
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
    await expect(card.getByRole("button", { name: "Choose a folder" })).toBeVisible({ timeout: 10_000 });
    // Named after its first words, never ending on a little one.
    await expect(card.locator(".turn-name")).toHaveText("A lighthouse at dusk");
    await card.getByRole("button", { name: "Choose a folder" }).click();
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
    // A restart starts a new chat, and the one before waits in the history, with what was asked
    // in it. (The preview's library doesn't outlive a reload, so its card says the picture has
    // gone, as the app does.)
    await expect(chat(page).getByRole("heading", { name: /what should your folder look like/i })).toBeVisible();
    await chat(page).getByRole("button", { name: "chats", exact: true }).click();
    const drawer = page.getByRole("complementary", { name: "chats" });
    await drawer.locator(".chat-open", { hasText: "Pop art cats" }).click();
    await expect(chat(page).locator(".studio-chat-title")).toHaveText("Pop art cats");
    await expect(chat(page).locator(".turn-ask")).toContainText("pop art cats");
    await expect(chat(page).getByText("This picture has been deleted from your library.")).toBeVisible();
    if (!(await drawer.isVisible())) await chat(page).getByRole("button", { name: "chats", exact: true }).click();
    await drawer.getByLabel("search chats").fill("lighthouse");
    await expect(drawer.getByText(/No chat is called anything like/)).toBeVisible();
    // Escape empties the search first, and closes the drawer only after that.
    await drawer.getByLabel("search chats").press("Escape");
    await expect(drawer.getByLabel("search chats")).toHaveValue("");
    await expect(drawer).toBeVisible();
    await expect(drawer.locator(".chat-open", { hasText: "Pop art cats" })).toBeVisible();
    await drawer.getByLabel("search chats").press("Escape");
    await expect(drawer).toBeHidden();
    await chat(page).getByRole("button", { name: "chats", exact: true }).click();
    await drawer.getByLabel("search chats").fill("cats");
    await drawer.getByRole("button", { name: "rename Pop art cats" }).click();
    await drawer.getByLabel("chat name").fill("Cats for the desktop");
    await drawer.getByLabel("chat name").press("Enter");
    await expect(chat(page).locator(".studio-chat-title")).toHaveText("Cats for the desktop");
    await drawer.getByRole("button", { name: "delete Cats for the desktop" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Delete" }).click();
    await expect(drawer.getByText(/Your chats will be here|No chat is called/)).toBeVisible();
  });

  test("starts a new chat each time it's opened, with the last one in the history", async ({ page }) => {
    await withKey(page);
    await sendIdea(page, "a paper boat");
    await expect(chat(page).locator(".turn-result:not(.is-developing)")).toHaveCount(1, { timeout: 10_000 });
    await openView(page, /all skins/i);
    await openView(page, /generate with ai/i);
    await expect(chat(page).getByRole("heading", { name: /what should your folder look like/i })).toBeVisible();
    await expect(chat(page).locator("article.turn")).toHaveCount(0);
    await chat(page).getByRole("button", { name: "chats", exact: true }).click();
    const drawer = page.getByRole("complementary", { name: "chats" });
    await expect(drawer.getByRole("button", { name: "rename A paper boat" })).toBeVisible();
    // Going back and forth without asking anything doesn't pile up empty chats.
    await openView(page, /all skins/i);
    await openView(page, /generate with ai/i);
    await chat(page).getByRole("button", { name: "chats", exact: true }).click();
    await expect(page.getByRole("complementary", { name: "chats" }).locator(".chat-open")).toHaveCount(1);
  });

  test("the local model can be removed, after asking, and set up again", async ({ page }) => {
    await openApp(page, { query: "localready" });
    await openView(page, /generate with ai/i);
    await chat(page).locator(".model-pill").click();
    await settings(page).getByRole("radio", { name: /Local Model/ }).click();
    await expect(localReady(page)).toBeVisible();
    await expect(settings(page).getByText(/Set up and ready|Nothing you make here leaves/)).toHaveCount(0);
    // The model is the fold's title; the machine, what it runs with, how long a picture took and
    // where it's stored fold open under it, the path veiled until it's pointed at.
    const model = settings(page).getByRole("button", { name: /FLUX\.2 \[klein\] 4B/ });
    await expect(model).toContainText("4-bit");
    await expect(model).toHaveAttribute("aria-expanded", "false");
    await model.click();
    await expect(model).toHaveAttribute("aria-expanded", "true");
    const facts = settings(page).getByRole("region", { name: "about the local model" });
    for (const text of ["Machine", "NVIDIA GeForce RTX 3050 Ti, 4 GB", "Runs with", "CUDA", "Stored at"]) await expect(facts).toContainText(text);
    await expect(facts).not.toContainText("Kept in");
    const path = facts.locator(".local-path");
    await expect(path).toHaveCSS("filter", /blur/);
    await path.hover();
    await expect(path).toHaveCSS("filter", "none");
    await facts.getByRole("button", { name: "Remove the model" }).click();
    const ask = page.getByRole("dialog", { name: "Remove the local model?" });
    await expect(ask).toContainText("5.4 GB");
    await ask.getByRole("button", { name: "Cancel" }).click();
    await expect(localReady(page)).toBeVisible();
    await facts.getByRole("button", { name: "Remove the model" }).click();
    await page.getByRole("dialog", { name: "Remove the local model?" }).getByRole("button", { name: "Remove" }).click();
    // Removed is shown by what comes next: it offers to set the model up again, and says no more.
    await expect(settings(page).getByRole("button", { name: "Set up the local model" })).toBeVisible();
    await expect(settings(page).getByText(/Removed|free again/)).toHaveCount(0);
    await expect(settings(page).getByRole("button", { name: "Remove the model" })).toHaveCount(0);
  });

  test("won't start setting up on a disk without room, and says to clear some", async ({ page }) => {
    await openApp(page, { query: "lowspace" });
    await openView(page, /generate with ai/i);
    await chat(page).locator(".model-pill").click();
    await settings(page).getByRole("radio", { name: /Local Model/ }).click();
    await expect(settings(page).getByText(/Clear some space first: setting it up wants 8\.1 GB free, and this disk has 3\.2 GB\./)).toBeVisible();
    await expect(settings(page).getByRole("button", { name: "Set up the local model" })).toBeDisabled();
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

  test("sets the local model up in one click, then paints with it, with its progress and log", async ({ page }) => {
    await openApp(page, { query: "aifail=memory" });
    await openView(page, /generate with ai/i);
    await chat(page).locator(".model-pill").click();
    await settings(page).getByRole("radio", { name: /Local Model/ }).click();
    await expect(settings(page).getByText("NVIDIA GeForce RTX 3050 Ti, 4 GB")).toBeVisible();
    await settings(page).getByRole("button", { name: "Set up the local model" }).click();
    await expect(settings(page).getByText(/Downloading what the local model needs/)).toBeVisible();
    // The Local Model's tile says it's downloading, and stops saying so once it's ready.
    const tile = settings(page).getByRole("radio", { name: /Local Model/ });
    await expect(tile.getByRole("img", { name: "downloading" })).toBeVisible();
    await expect(localReady(page)).toBeVisible({ timeout: 10_000 });
    await expect(tile.getByRole("img", { name: "downloading" })).toHaveCount(0);
    await settings(page).getByRole("button", { name: "close" }).click();
    await expect(chat(page).locator(".studio-foot")).toContainText("generated right here on your machine");
    await sendIdea(page, "a paper boat");
    const card = chat(page).locator("article.turn").last();
    await expect(card.locator(".turn-where")).toContainText(/Step \d of 4/, { timeout: 10_000 });
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
    // The app opens on a new chat; the saved ones are in the history.
    await expect(chat(page).getByRole("heading", { name: /what should your folder look like/i })).toBeVisible();
    await chat(page).getByRole("button", { name: "chats", exact: true }).click();
    const drawer = page.getByRole("complementary", { name: "chats" });
    await expect(drawer.getByRole("button", { name: "rename A lighthouse" })).toBeVisible();
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

  test("a new chat starts without the last one's reference pictures", async ({ page }) => {
    await withKey(page);
    await expect(chat(page).locator(".studio-foot")).toContainText("billed to your");
    await sendIdea(page, "a paper boat");
    await expect(chat(page).locator(".studio-foot")).toHaveCount(0);
    await chat(page).getByRole("button", { name: "add a reference picture" }).click();
    await expect(chat(page).getByRole("button", { name: "remove Reference.jpg" })).toBeVisible();
    await chat(page).getByRole("button", { name: "new chat" }).click();
    await expect(chat(page).getByRole("heading", { name: /what should your folder look like/i })).toBeVisible();
    await expect(chat(page).getByRole("button", { name: "remove Reference.jpg" })).toHaveCount(0);
    await expect(chat(page).locator(".studio-foot")).toContainText("billed to your");
  });

  test("the local model paints one picture at a time, whichever chat asks", async ({ page }) => {
    await openApp(page, { query: "localready" });
    await openView(page, /generate with ai/i);
    await expect(chat(page).locator(".studio-foot")).toContainText("generated right here on your machine");
    await sendIdea(page, "a paper boat");
    // In a chat the box sits at the bottom with nothing under it.
    await expect(chat(page).locator(".studio-foot")).toHaveCount(0);
    await expect(chat(page).locator("article.turn").last().getByRole("button", { name: "Stop" })).toBeVisible();
    await chat(page).getByRole("button", { name: "new chat" }).click();
    await box(page).fill("a lighthouse at dusk");
    const generate = chat(page).getByRole("button", { name: "generate" });
    await expect(generate).toBeDisabled();
    await expect(generate).toHaveAttribute("data-tip", "The local model is still painting the last one");
    // Free again once the first one is done.
    await expect(generate).toBeEnabled({ timeout: 10_000 });
  });

  test("opening a picture's details leaves the picture where it is", async ({ page }) => {
    await openApp(page, { query: "localready" });
    await openView(page, /generate with ai/i);
    await sendIdea(page, "a paper boat");
    const card = chat(page).locator("article.turn").last();
    await expect(card.getByRole("button", { name: "Choose a folder" })).toBeVisible({ timeout: 10_000 });
    // Where the picture sits in its card: the chat may scroll as the card grows, the picture mustn't move in it.
    const offset = () =>
      card.evaluate((c) => {
        const img = c.querySelector(".turn-img")!.getBoundingClientRect();
        const box = c.getBoundingClientRect();
        return `${Math.round(img.top - box.top)},${Math.round(img.left - box.left)}`;
      });
    await page.waitForTimeout(1000); // the picture's entrance
    const closed = await offset();
    await card.getByRole("button", { name: "Details" }).click();
    await expect(card.locator(".turn-log-lines")).toBeVisible();
    expect(await offset()).toBe(closed);
    await card.getByRole("button", { name: "Hide the details" }).click();
    await expect(card.locator(".turn-log-lines")).toHaveCount(0);
    expect(await offset()).toBe(closed);
  });

  test("coming back keeps the folder chosen in the library meanwhile", async ({ page }) => {
    await openApp(page);
    await openView(page, /generate with ai/i);
    await chat(page).getByRole("button", { name: "Choose a folder" }).click();
    await expect(folderPanel(page).getByRole("heading", { name: "Projects" })).toBeVisible();
    await openView(page, /all skins/i);
    await folderPanel(page).getByRole("button", { name: "choose a different folder than Projects" }).click();
    await expect(folderPanel(page).getByRole("heading", { name: "Wedding" })).toBeVisible();
    await openView(page, /generate with ai/i);
    await expect(chat(page).getByRole("button", { name: /for the folder Wedding/ })).toBeVisible();
    await page.waitForTimeout(1000);
    await expect(folderPanel(page).getByRole("heading", { name: "Wedding" })).toBeVisible();
    await expect(chat(page).getByRole("button", { name: /for the folder Wedding/ })).toBeVisible();
  });

  test("setting the local model up can be stopped, and carried on", async ({ page }) => {
    await openApp(page);
    await openView(page, /generate with ai/i);
    await chat(page).locator(".model-pill").click();
    await settings(page).getByRole("radio", { name: /Local Model/ }).click();
    await settings(page).getByRole("button", { name: "Set up the local model" }).click();
    await expect(settings(page).getByText(/Downloading what the local model needs/)).toBeVisible();
    await settings(page).getByRole("button", { name: "Stop" }).click();
    // A stop asked for isn't an error, and there is one way on from it, not two.
    await expect(settings(page).getByRole("status").filter({ hasText: "What was downloaded is kept" })).toBeVisible();
    await expect(settings(page).getByRole("alert")).toHaveCount(0);
    await expect(settings(page).getByRole("button", { name: "Set up the local model" })).toHaveCount(0);
    await settings(page).getByRole("button", { name: "Carry on setting up" }).click();
    await expect(localReady(page)).toBeVisible({ timeout: 10_000 });
  });

  test("before it is set up, the Local Model says so, and setting up counts the whole download", async ({ page }) => {
    await openApp(page);
    await openView(page, /generate with ai/i);
    await chat(page).locator(".model-pill").click();
    const tile = settings(page).getByRole("radio", { name: /Local Model/ });
    await expect(tile).toContainText("Not set up");
    await tile.click();
    // How long a picture takes is only known once one has been painted, and it says so.
    await expect(settings(page).locator(".local-list")).toContainText("Last pictureShows after your first one");
    await settings(page).getByRole("button", { name: "Set up the local model" }).click();
    await expect(tile.getByRole("img", { name: "downloading" })).toBeVisible();
    // Each file as it comes, and how far the whole download has got.
    await expect(settings(page).locator(".local-progress")).toContainText(/of \d+(\.\d+)? GB in all/);
    // Ready shows on the tile, and nothing else needs to say so.
    await expect(localReady(page)).toBeVisible({ timeout: 10_000 });
    await expect(settings(page).getByText(/Set up and ready/)).toHaveCount(0);
  });

  test("a setup under way is found again when its settings are opened again", async ({ page }) => {
    await openApp(page, { query: "slowsetup" });
    await openView(page, /generate with ai/i);
    await chat(page).locator(".model-pill").click();
    await settings(page).getByRole("radio", { name: /Local Model/ }).click();
    await settings(page).getByRole("button", { name: "Set up the local model" }).click();
    await expect(settings(page).getByText(/Downloading what the local model needs/)).toBeVisible();
    await settings(page).getByRole("button", { name: "close" }).click();
    await chat(page).locator(".model-pill").click();
    await settings(page).getByRole("radio", { name: /Local Model/ }).click();
    // Where it has got to, with its Stop; it isn't offered as if nothing were running.
    await expect(settings(page).getByText(/Downloading what the local model needs/)).toBeVisible();
    await expect(settings(page).getByRole("button", { name: "Set up the local model" })).toHaveCount(0);
    await settings(page).getByRole("button", { name: "Stop" }).click();
    await expect(settings(page).getByRole("status").filter({ hasText: "What was downloaded is kept" })).toBeVisible();
    await settings(page).getByRole("button", { name: "Carry on setting up" }).click();
    await expect(localReady(page)).toBeVisible({ timeout: 20_000 });
  });

  test("a key that doesn't pass its check is still saved, and says so", async ({ page }) => {
    await withKey(page);
    await chat(page).locator(".model-pill").click();
    await expect(settings(page).getByRole("radio", { name: /OpenAI/ }).getByLabel("Key saved")).toBeVisible();
  });
});
