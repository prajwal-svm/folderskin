import { expect, test, type Page } from "@playwright/test";
import { letGo, openApp } from "./app";

// The preview's first folder is Projects: 28 folders inside, one branch six levels down
// (Research, Papers, 2026, Drafts, Figures, Final). The second is Wedding, which wears an icon of
// its own, as does Guests inside it. The fourth is Photo archive: 48,210 folders, thirty years of
// twelve months of days, whose count `?holdcount` holds at about 12,400 until `mockCountGo()`.
// `?bigtree` puts a made-up Studio first: 4,960 folders, 1,400 of them in Camera roll and a branch
// 37 levels deep (src/lib/devMock.ts).

const panel = (page: Page) => page.locator(".right-slot");
const chooser = (page: Page) => page.getByRole("dialog", { name: "Choose subfolders" });
const tree = (page: Page) => chooser(page).getByRole("tree");
const item = (page: Page, name: string) => tree(page).getByRole("treeitem", { name, exact: true });
/** The column listing the folders inside `parent`. */
const column = (page: Page, parent: string) => tree(page).getByRole("group", { name: parent, exact: true });
const names = (page: Page, parent: string) => column(page, parent).getByRole("treeitem").allTextContents();
const box = (page: Page, name: string) => item(page, name).locator(".fsc-hit");
const count = (page: Page) => chooser(page).locator(".fsc-count");
const includeSwitch = (page: Page) => panel(page).getByRole("switch", { name: /Include subfolders/ });
const apply = (page: Page) => panel(page).getByRole("button", { name: /^Apply to \d+ folders$/ });

/** Chooses the preview's next folder and includes its subfolders, with a skin picked for it. */
async function includeSubfolders(page: Page, { query = "", skin = true } = {}) {
  await openApp(page, { query });
  await panel(page).getByRole("button", { name: /^choose a folder from/ }).click();
  if (skin) await page.locator(".tile-hit").first().click();
  await includeSwitch(page).click();
  await expect(includeSwitch(page)).toHaveAttribute("aria-checked", "true");
}

async function openChooser(page: Page) {
  await panel(page).getByRole("button", { name: "Choose subfolders" }).click();
  await expect(chooser(page)).toBeVisible();
  await expect(tree(page)).toBeFocused();
  await expect(tree(page).getByRole("treeitem").first()).toBeVisible();
}

/** Whether the folder is the one the keys move from. */
const current = (page: Page, name: string) => expect(item(page, name)).toHaveClass(/is-current/);

test.describe("choosing subfolders", () => {
  test("opens on the folders inside, every one ticked, as Finder's columns", async ({ page }) => {
    await includeSubfolders(page);
    await expect(includeSwitch(page)).toContainText("28 folders inside");
    await expect(apply(page)).toHaveText("Apply to 29 folders");
    await openChooser(page);
    await expect(chooser(page)).toContainText("Tick the folders inside Projects to include.");
    expect(await names(page, "Projects")).toEqual(["Clients", "Design", "Invoices", "Notes", "Photos", "Research", "Templates", "Videos"]);
    await expect(tree(page).getByRole("treeitem", { checked: false })).toHaveCount(0);
    await expect(count(page)).toHaveText("28 of 28 folders chosen");
    // The first folder is highlighted, and the next column shows what's inside it.
    await current(page, "Clients");
    await expect(item(page, "Clients")).toHaveAttribute("aria-expanded", "true");
    expect(await names(page, "Clients")).toEqual(["Acme", "Globex", "Initech"]);
    await expect(item(page, "Notes")).not.toHaveAttribute("aria-expanded");
    await expect(chooser(page).getByRole("button", { name: "Select all" })).toBeDisabled();
  });

  test("goes six levels down with the keyboard, the columns keeping the newest in view", async ({ page }) => {
    await includeSubfolders(page);
    await openChooser(page);
    for (let i = 0; i < 5; i++) await page.keyboard.press("ArrowDown");
    await current(page, "Research");
    for (const name of ["Papers", "2026", "Drafts", "Figures", "Final"]) {
      await page.keyboard.press("ArrowRight");
      await current(page, name);
    }
    await expect(item(page, "Final")).toHaveAttribute("aria-level", "6");
    // Six columns and Final's own, more than the five that fit: the first has scrolled away.
    await expect(tree(page).getByRole("group")).toHaveCount(6);
    await expect(item(page, "Final")).toBeInViewport();
    await expect(chooser(page).locator(".fsc-leaf")).toBeInViewport();
    await expect(column(page, "Projects")).not.toBeInViewport();
    const path = chooser(page).getByRole("navigation", { name: "folder path" });
    await expect(path.locator("[aria-current=location]")).toHaveText("Final");
    await expect(path).toContainText("Projects");

    // Right does nothing at the bottom, and left goes back out, one level at a time.
    await page.keyboard.press("ArrowRight");
    await current(page, "Final");
    for (let i = 0; i < 5; i++) await page.keyboard.press("ArrowLeft");
    await current(page, "Research");
    await expect(item(page, "Research")).toHaveAttribute("aria-level", "1");
    await expect(column(page, "Projects")).toBeInViewport();
    await page.keyboard.press("ArrowLeft");
    await current(page, "Research");
    // Home, End, and a name typed.
    await page.keyboard.press("End");
    await current(page, "Videos");
    await page.keyboard.press("Home");
    await current(page, "Clients");
    await page.keyboard.type("te");
    await current(page, "Templates");
  });

  test("ticks and clears a folder with everything inside it, with a dash on the ones above", async ({ page }) => {
    await includeSubfolders(page);
    await openChooser(page);
    for (let i = 0; i < 5; i++) await page.keyboard.press("ArrowDown");
    for (let i = 0; i < 5; i++) await page.keyboard.press("ArrowRight");
    await current(page, "Final");
    // Space clears the highlighted folder, and every folder it's in shows a dash.
    await page.keyboard.press("Space");
    await expect(item(page, "Final")).toHaveAttribute("aria-checked", "false");
    for (const name of ["Figures", "Drafts", "2026", "Papers", "Research"]) await expect(item(page, name)).toHaveAttribute("aria-checked", "mixed");
    await expect(count(page)).toHaveText("27 of 28 folders chosen");

    // A box clicked ticks or clears without moving: Clients and the four folders inside it.
    for (let i = 0; i < 5; i++) await page.keyboard.press("ArrowLeft");
    await box(page, "Clients").click();
    await current(page, "Research");
    await expect(item(page, "Clients")).toHaveAttribute("aria-checked", "false");
    await expect(count(page)).toHaveText("22 of 28 folders chosen");
    await item(page, "Clients").click();
    await current(page, "Clients");
    expect(await column(page, "Clients").getByRole("treeitem", { checked: false }).count()).toBe(3);
    await item(page, "Acme").click();
    await expect(item(page, "Contracts")).toHaveAttribute("aria-checked", "false");

    // A folder with a dash is ticked whole when its box is clicked, and cleared the next time.
    await box(page, "Research").click();
    await expect(item(page, "Research")).toHaveAttribute("aria-checked", "true");
    await expect(count(page)).toHaveText("23 of 28 folders chosen");
    await box(page, "Research").click();
    await expect(item(page, "Research")).toHaveAttribute("aria-checked", "false");
    await expect(count(page)).toHaveText("17 of 28 folders chosen");

    await chooser(page).getByRole("button", { name: "Select none" }).click();
    await expect(count(page)).toHaveText("0 of 28 folders chosen");
    await expect(tree(page).getByRole("treeitem", { checked: true })).toHaveCount(0);
    await expect(tree(page)).toBeFocused();
    await chooser(page).getByRole("button", { name: "Select all" }).click();
    await expect(count(page)).toHaveText("28 of 28 folders chosen");
    await expect(tree(page).getByRole("treeitem", { checked: false })).toHaveCount(0);
  });

  test("puts the choice in the panel, on the Apply button, and on the run", async ({ page }) => {
    await includeSubfolders(page);
    await openChooser(page);
    await box(page, "Clients").click();
    await item(page, "Photos").click();
    await box(page, "2020").click();
    await expect(count(page)).toHaveText("22 of 28 folders chosen");
    await chooser(page).getByRole("button", { name: "Done" }).click();
    await expect(chooser(page)).toBeHidden();
    await expect(includeSwitch(page)).toContainText("22 of 28 folders inside");
    await expect(apply(page)).toHaveText("Apply to 23 folders");

    // Asked first, in the choice's own words, then only those folders are changed.
    await apply(page).click();
    const ask = page.getByRole("dialog", { name: /to 23 folders\?$/ });
    await expect(ask).toContainText("Projects and the 22 folders you chose inside it get this skin");
    await ask.getByRole("button", { name: "Apply to 23 folders" }).click();
    await expect(panel(page).getByRole("status").filter({ hasText: /folders now wear/ })).toContainText("23 folders now wear", { timeout: 15_000 });

    // The choice is still there when the chooser opens again, and when the switch comes back on.
    await openChooser(page);
    await expect(item(page, "Clients")).toHaveAttribute("aria-checked", "false");
    await expect(item(page, "Photos")).toHaveAttribute("aria-checked", "mixed");
    await expect(count(page)).toHaveText("22 of 28 folders chosen");
    await page.keyboard.press("Escape");
  });

  test("keeps the choice while the switch is off, and starts over for another folder", async ({ page }) => {
    await includeSubfolders(page);
    await openChooser(page);
    await box(page, "Clients").click();
    await page.keyboard.press("Enter");
    await expect(chooser(page)).toBeHidden();
    await expect(includeSwitch(page)).toContainText("23 of 28 folders inside");
    await includeSwitch(page).click();
    await expect(includeSwitch(page)).toContainText("28 folders inside");
    await expect(panel(page).getByRole("button", { name: "Choose subfolders" })).toHaveCount(0);
    await expect(panel(page).getByRole("button", { name: "Apply skin" })).toBeVisible();
    await includeSwitch(page).click();
    await expect(includeSwitch(page)).toContainText("23 of 28 folders inside");

    await panel(page).getByRole("button", { name: "Choose a different folder", exact: true }).click();
    await expect(panel(page).getByRole("heading", { name: "Wedding" })).toBeVisible();
    await includeSwitch(page).click();
    await expect(includeSwitch(page)).toContainText("6 folders inside");
  });

  test("with none ticked, the folder goes on its own", async ({ page }) => {
    await includeSubfolders(page);
    await openChooser(page);
    await chooser(page).getByRole("button", { name: "Select none" }).click();
    await chooser(page).getByRole("button", { name: "Done" }).click();
    await expect(includeSwitch(page)).toHaveAttribute("aria-checked", "false");
    await expect(apply(page)).toHaveCount(0);
    await expect(panel(page).getByRole("button", { name: "Apply skin" })).toBeVisible();
  });

  test("Escape and Cancel leave everything as it was", async ({ page }) => {
    await includeSubfolders(page);
    await openChooser(page);
    await box(page, "Clients").click();
    await expect(count(page)).toHaveText("23 of 28 folders chosen");
    await page.keyboard.press("Escape");
    await expect(chooser(page)).toBeHidden();
    await expect(includeSwitch(page)).toContainText("28 folders inside");
    await expect(apply(page)).toHaveText("Apply to 29 folders");

    await openChooser(page);
    await expect(item(page, "Clients")).toHaveAttribute("aria-checked", "true");
    await box(page, "Design").click();
    await chooser(page).getByRole("button", { name: "Cancel" }).click();
    await expect(chooser(page)).toBeHidden();
    await expect(apply(page)).toHaveText("Apply to 29 folders");
  });

  test("takes the icons off only the folders chosen, and leaves alone the ones without one", async ({ page }) => {
    await openApp(page);
    // Wedding, which wears an icon of its own, and so does Guests inside it.
    await panel(page).getByRole("button", { name: /^choose a folder from/ }).click();
    await panel(page).getByRole("button", { name: "choose a different folder than Projects" }).click();
    await expect(panel(page).getByRole("heading", { name: "Wedding" })).toBeVisible();
    await includeSwitch(page).click();
    await openChooser(page);
    await chooser(page).getByRole("button", { name: "Select none" }).click();
    await box(page, "Guests").click();
    await box(page, "Ceremony").click();
    await expect(count(page)).toHaveText("3 of 6 folders chosen");
    await chooser(page).getByRole("button", { name: "Done" }).click();

    await panel(page).getByRole("button", { name: "Remove custom icons" }).click();
    const ask = page.getByRole("dialog", { name: "Remove the custom icons from 4 folders?" });
    await expect(ask).toContainText("Wedding and the 3 folders you chose inside it go back to the default folder icon");
    await ask.getByRole("button", { name: "Remove the icons" }).click();
    const summary = panel(page).getByRole("status").filter({ hasText: /default icon back/ });
    await expect(summary).toContainText("2 folders have the default icon back", { timeout: 15_000 });
    await expect(summary).toContainText("2 folders had no icon of their own.");
  });
});

test.describe("choosing among thousands of subfolders", () => {
  test("opens the whole tree, and a column of 1,400 folders scrolls at once, drawing only what's in view", async ({ page }) => {
    await includeSubfolders(page, { query: "bigtree" });
    await expect(includeSwitch(page)).toContainText("4,960 folders inside");
    await openChooser(page);
    await expect(count(page)).toHaveText("4,960 of 4,960 folders chosen");
    await current(page, "Camera roll");
    const days = column(page, "Camera roll");
    await expect(days.getByRole("treeitem").first()).toHaveAttribute("aria-setsize", "1400");
    // Finder's order: Day 2 before Day 10.
    expect((await days.getByRole("treeitem").allTextContents()).slice(0, 3)).toEqual(["Day 1", "Day 2", "Day 3"]);
    expect(await days.getByRole("treeitem").count()).toBeLessThan(60);

    // The whole column, a screenful at a time from top to bottom: each is drawn in a frame or two,
    // and the rows drawn stay a screenful.
    const scrolled = await days.evaluate(async (el) => {
      const frames = () => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));
      const started = performance.now();
      let jumps = 0;
      let most = 0;
      for (let top = 0; top < el.scrollHeight; top += el.clientHeight) {
        el.scrollTop = top;
        await frames();
        jumps += 1;
        most = Math.max(most, el.querySelectorAll('[role="treeitem"]').length);
      }
      el.scrollTop = el.scrollHeight;
      await frames();
      const names = [...el.querySelectorAll('[role="treeitem"]')].map((row) => row.textContent);
      return { ms: performance.now() - started, jumps, most, names };
    });
    expect(scrolled.names).toContain("Day 1400");
    expect(scrolled.most).toBeLessThan(60);
    expect(scrolled.jumps).toBeGreaterThan(50);
    expect(scrolled.ms / scrolled.jumps).toBeLessThan(120);

    // The keys too: into the column, to its end, and a page back up.
    await page.keyboard.press("ArrowRight");
    await current(page, "Day 1");
    await page.keyboard.press("End");
    await current(page, "Day 1400");
    await expect(item(page, "Day 1400")).toBeInViewport();
    await page.keyboard.press("PageUp");
    const highlighted = tree(page).locator(".fsc-row.is-current");
    await expect(highlighted).not.toHaveText("Day 1400");
    await expect(highlighted).toBeInViewport();
    await page.keyboard.press("Space");
    await expect(count(page)).toHaveText("4,959 of 4,960 folders chosen");
    await expect(item(page, "Camera roll")).toHaveAttribute("aria-checked", "mixed");
  });

  test("goes 37 levels down a branch and keeps the deepest column in view", async ({ page }) => {
    await includeSubfolders(page, { query: "bigtree" });
    await openChooser(page);
    await page.keyboard.type("dee");
    await current(page, "Deep dive");
    for (let i = 0; i < 36; i++) await page.keyboard.press("ArrowRight");
    await current(page, "Level 36");
    await expect(item(page, "Level 36")).toHaveAttribute("aria-level", "37");
    await expect(item(page, "Level 36")).toBeInViewport();
    await expect(column(page, "Level 30")).not.toBeInViewport();
    await expect(chooser(page).getByRole("navigation", { name: "folder path" }).locator("[aria-current=location]")).toHaveText("Level 36");
    await page.keyboard.press("Space");
    await expect(item(page, "Level 35")).toHaveAttribute("aria-checked", "mixed");
    await expect(count(page)).toHaveText("4,959 of 4,960 folders chosen");
  });
});

test.describe("choosing among folders still being counted", () => {
  test("opens at once, and says how many are chosen as the count gets to them", async ({ page }) => {
    await openApp(page, { query: "holdcount" });
    for (let i = 0; i < 4; i++) await panel(page).getByRole("button", { name: /^choose a (folder from|different folder than)/ }).click();
    await expect(panel(page).getByRole("heading", { name: "Photo archive" })).toBeVisible();
    await page.locator(".tile-hit").first().click();
    await expect(includeSwitch(page)).toContainText(/[\d,]+ folders inside so far/);
    await includeSwitch(page).click();
    await openChooser(page);
    await expect(column(page, "Photo archive").getByRole("treeitem").first()).toHaveAttribute("aria-setsize", "30");
    expect((await names(page, "Photo archive")).slice(0, 2)).toEqual(["1997", "1998"]);
    await expect(count(page)).toHaveText(/^[\d,]+ of [\d,]+ folders chosen so far$/);
    // A year is its twelve months and their days: 1,609 folders, all counted by now.
    await box(page, "1997").click();
    await expect(item(page, "1997")).toHaveAttribute("aria-checked", "false");
    await expect(count(page)).toHaveText(/^[\d,]+ of [\d,]+ folders chosen so far$/);
    // Each column is read as it opens, in Finder's order.
    await item(page, "1998").click();
    await expect(column(page, "1998").getByRole("treeitem").first()).toHaveText("April");
    await chooser(page).getByRole("button", { name: "Done" }).click();
    await expect(includeSwitch(page)).toContainText(/[\d,]+ of [\d,]+ folders inside so far/);

    // Once every folder is counted, the numbers are whole, in the panel and on Apply.
    await letGo(page, "mockCountGo");
    await expect(includeSwitch(page)).toContainText("46,601 of 48,210 folders inside");
    await expect(panel(page).getByRole("button", { name: "Apply to 46,602 folders" })).toBeVisible();
    await openChooser(page);
    await expect(count(page)).toHaveText("46,601 of 48,210 folders chosen");
  });
});
