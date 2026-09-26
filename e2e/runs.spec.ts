import { expect, test, type Page } from "@playwright/test";
import { letGo, openApp, openView } from "./app";

// A run over a folder's tree goes on in the background (src-tauri/src/tree/job.rs), shown in the
// sidebar on every view. The preview's fourth folder is Photo archive, 48,210 folders counted over
// a moment (src/lib/devMock.ts): `?holdcount` holds its count at about 12,400 until
// `mockCountGo()`, `?holdrun` holds a run after its first folders (3,120 of a big one) until
// `mockRunGo()`, and `?unfocused` has the window behind others when a run ends.

const panel = (page: Page) => page.locator(".right-slot");
const dock = (page: Page) => page.getByRole("region", { name: "run over a folder and the folders inside it" });
const includeSwitch = (page: Page) => panel(page).getByRole("switch", { name: /Include subfolders/ });
const pickNext = (page: Page) => panel(page).getByRole("button", { name: /^choose a (folder from|different folder than)/ }).click();

/** Photo archive, with a skin picked for it: Projects, Wedding and Taxes 2026 come first. */
async function photoArchive(page: Page, query: string) {
  await openApp(page, { query });
  for (let i = 0; i < 4; i++) await pickNext(page);
  await expect(panel(page).getByRole("heading", { name: "Photo archive" })).toBeVisible();
  await page.locator(".tile-hit").first().click();
}

test.describe("a run over a big tree", () => {
  test("counts the folders inside as it goes, without holding anything up", async ({ page }) => {
    await photoArchive(page, "holdcount");
    await expect(includeSwitch(page)).toContainText(/^Include subfolders[\d,]+ folders inside so far$/);
    await includeSwitch(page).click();
    await expect(includeSwitch(page)).toHaveAttribute("aria-checked", "true");
    await expect(panel(page).getByRole("button", { name: /^Apply to [\d,]+\+ folders$/ })).toBeVisible();

    // Asked first, with the count so far, the space it takes, and that it goes on in the background.
    await panel(page).getByRole("button", { name: /^Apply to/ }).click();
    const ask = page.getByRole("dialog", { name: /^Apply .+ to [\d,]+\+ folders\?$/ });
    await expect(ask).toContainText(/So far FolderSkin has found [\d,]+ folders inside Photo archive, and it's still counting\./);
    await expect(ask).toContainText(/about 2\.7 MB, so at least [\d.]+ GB in all\./);
    await expect(ask).toContainText("It runs in the background while you do other things");
    // The count finishes while the question is open, and the question says so.
    await letGo(page, "mockCountGo");
    await expect(page.getByRole("dialog", { name: /to 48,211 folders\?$/ })).toContainText("Photo archive and the 48,210 folders inside it get this skin");
    await page.getByRole("dialog").getByRole("button", { name: "Cancel" }).click();
    await expect(includeSwitch(page)).toContainText("48,210 folders inside");
  });

  test("started before the count is done, reads 12,000+ until the count or the run has found them all", async ({ page }) => {
    await photoArchive(page, "holdcount&holdrun");
    await includeSwitch(page).click();
    await panel(page).getByRole("button", { name: /^Apply to [\d,]+\+ folders$/ }).click();
    await page.getByRole("dialog").getByRole("button", { name: /^Apply to/ }).click();
    await expect(dock(page)).toContainText(/3,120 of [\d,]+\+/);
    await expect(panel(page).getByRole("status").filter({ hasText: "Applying" })).toContainText(/3,120 of [\d,]+\+/);
    // The count finishes first, and says how many there are.
    await letGo(page, "mockCountGo");
    await expect(includeSwitch(page)).toContainText("48,210 folders inside");
    await expect(dock(page)).toContainText("3,120 of 48,211");
    await letGo(page, "mockRunGo");
    await expect(dock(page)).toContainText("48,211 folders now wear", { timeout: 20_000 });
  });

  test("goes on in the background, shown in the sidebar wherever the window is, and says when it's done", async ({ page }) => {
    await photoArchive(page, "holdrun&unfocused");
    await includeSwitch(page).click();
    await expect(includeSwitch(page)).toContainText("48,210 folders inside");
    await panel(page).getByRole("button", { name: "Apply to 48,211 folders" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Apply to 48,211 folders" }).click();

    // The panel and the sidebar both follow it, out of the 48,211 the count found before it
    // started, while the run's own walk is still finding them.
    await expect(dock(page)).toContainText(/Applying .+Photo archive/);
    await expect(dock(page)).toContainText("3,120 of 48,211");
    await expect(panel(page).getByRole("status").filter({ hasText: "Applying" })).toContainText("3,120 of 48,211");
    await expect(panel(page)).toContainText("It carries on while you do other things.");
    // Above the folder's name, what's happening to it rather than a skin being tried on.
    await expect(panel(page).locator(".stage-eyebrow")).toHaveText(/^Applying .+/);

    // Other views show it too.
    await openView(page, /community/i);
    await expect(dock(page)).toContainText(/3,120 of/);
    await openView(page, /all skins/i);

    // Another folder can be picked meanwhile, but another run waits its turn.
    await pickNext(page);
    await expect(panel(page).getByRole("heading", { name: "Projects" })).toBeVisible();
    await includeSwitch(page).click();
    await panel(page).getByRole("button", { name: "Apply to 29 folders" }).click();
    await expect(panel(page).getByRole("alert")).toHaveText("Wait for the run in Photo archive to finish, or stop it, before starting another.");
    await expect(page.getByRole("dialog")).toHaveCount(0);

    // The sidebar brings the run's folder back.
    await dock(page).getByRole("button", { name: "show Photo archive in the folder panel" }).click();
    await expect(panel(page).getByRole("heading", { name: "Photo archive" })).toBeVisible();
    await expect(panel(page).getByRole("status").filter({ hasText: "Applying" })).toBeVisible();

    await letGo(page, "mockRunGo");
    await expect(dock(page)).toContainText(/48,211 folders now wear/, { timeout: 20_000 });
    await expect(panel(page).getByRole("status").filter({ hasText: /now wear/ })).toContainText("48,211 folders now wear");
    await expect(panel(page).getByRole("button", { name: "Revert all 48,211" })).toBeVisible();
    // The window was behind others, so it said so as a notification.
    const notified = await page.evaluate(() => (window as unknown as { mockNotifications?: { title: string }[] }).mockNotifications ?? []);
    expect(notified.map((n) => n.title)).toEqual([expect.stringMatching(/^.+ is on 48,211 folders in Photo archive$/)]);

    // Put away in the sidebar, it's put away in the panel too.
    await dock(page).getByRole("button", { name: "hide this summary" }).click();
    await expect(dock(page)).toHaveCount(0);
    await expect(panel(page).getByRole("status").filter({ hasText: /now wear/ })).toHaveCount(0);
  });

  test("says another run has to wait, and stops saying so once the first has ended", async ({ page }) => {
    await photoArchive(page, "holdrun");
    await includeSwitch(page).click();
    await expect(includeSwitch(page)).toContainText("48,210 folders inside");
    await panel(page).getByRole("button", { name: "Apply to 48,211 folders" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Apply to 48,211 folders" }).click();
    await expect(dock(page)).toContainText(/3,120 of/);
    await pickNext(page);
    await includeSwitch(page).click();
    await panel(page).getByRole("button", { name: "Apply to 29 folders" }).click();
    const busy = panel(page).getByRole("alert");
    await expect(busy).toHaveText("Wait for the run in Photo archive to finish, or stop it, before starting another.");
    await letGo(page, "mockRunGo");
    await expect(dock(page)).toContainText("48,211 folders now wear", { timeout: 20_000 });
    await expect(busy).toHaveCount(0);
    await panel(page).getByRole("button", { name: "Apply to 29 folders" }).click();
    await expect(page.getByRole("dialog", { name: /to 29 folders\?$/ })).toBeVisible();
  });

  test("stops, carries on and is undone from the sidebar", async ({ page }) => {
    await openApp(page, { query: "bigtree&holdrun" });
    await pickNext(page);
    await page.locator(".tile-hit").first().click();
    await includeSwitch(page).click();
    await panel(page).getByRole("button", { name: "Apply to 4,961 folders" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Apply to 4,961 folders" }).click();
    await expect(dock(page)).toContainText(/3,120 of/);
    await dock(page).getByRole("button", { name: "Stop" }).click();
    await expect(dock(page)).toContainText("Stopped after 3,120 folders");
    await expect(dock(page)).toContainText(/Those 3,120 wear .+\. (The rest weren't reached|[\d,]+ weren't reached)\./);
    await expect(panel(page).getByRole("button", { name: "Revert these 3,120" })).toBeVisible();

    await dock(page).getByRole("button", { name: "Carry on" }).click();
    await expect(dock(page)).toContainText("4,961 folders now wear", { timeout: 20_000 });
    await expect(dock(page).getByRole("button", { name: "Carry on" })).toHaveCount(0);

    await dock(page).getByRole("button", { name: "Undo" }).click();
    await expect(dock(page)).toContainText("4,961 folders have the default icon back", { timeout: 20_000 });
    await expect(panel(page).getByRole("status").filter({ hasText: /default icon back/ })).toBeVisible();
    await expect(dock(page).getByRole("button", { name: "Undo" })).toHaveCount(0);
  });

  test("keeps the run in the folded sidebar, as a ring that brings its folder back", async ({ page }) => {
    await photoArchive(page, "holdrun");
    await includeSwitch(page).click();
    await expect(includeSwitch(page)).toContainText("48,210 folders inside");
    await panel(page).getByRole("button", { name: "Apply to 48,211 folders" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Apply to 48,211 folders" }).click();
    await expect(dock(page)).toContainText(/3,120 of/);
    await page.getByRole("button", { name: "collapse the sidebar" }).click();
    const ring = page.getByRole("button", { name: /^Applying .+ in Photo archive, 3,120 of 48,211$/ });
    await expect(ring).toBeVisible();
    await pickNext(page);
    await expect(panel(page).getByRole("heading", { name: "Projects" })).toBeVisible();
    await ring.click();
    await expect(panel(page).getByRole("heading", { name: "Photo archive" })).toBeVisible();
    await letGo(page, "mockRunGo");
  });
});
