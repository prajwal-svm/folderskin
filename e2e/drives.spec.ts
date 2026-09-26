import { expect, test, type Page } from "@playwright/test";
import { openApp, openView } from "./app";

// `?drive=external,network` makes the preview's first folders drives of those kinds
// (src/lib/devMock.ts). On the Mac, Backup Disk is an external drive with five folders on it,
// Studio NAS a network share, Holiday 2019 a disc, which is read-only, and Macintosh HD the
// startup disk. `?driveskins=2` adds two finished drives to the library, older than the rest.

const panel = (page: Page) => page.locator(".right-slot");
const title = (page: Page) => panel(page).locator(".stage-title");
const where = (page: Page) => panel(page).locator(".stage-path");
const status = (page: Page) => panel(page).locator(".stage-status");
const stageImage = (page: Page) => panel(page).locator(".stage-img").last();
const tiles = (page: Page) => page.locator(".gallery .tile");
const names = (page: Page) => tiles(page).locator(".tile-name-text").allTextContents();

/** Opens the preview with `query` and chooses its first folder, a drive. */
async function pickDrive(page: Page, query: string) {
  await openApp(page, { query });
  await panel(page).getByRole("button", { name: /^choose a folder from/ }).click();
}

test.describe("a drive picked instead of a folder", () => {
  test("says which drive it is and what kind", async ({ page }) => {
    await pickDrive(page, "drive=external,network");
    await expect(title(page)).toHaveText("Backup Disk");
    await expect(where(page)).toHaveText("External drive · /Volumes/Backup Disk");
    await expect(where(page).locator(".stage-kind")).toHaveText("External drive");
    // With no icon of its own it shows as the plain drive.
    await expect(stageImage(page)).toHaveAttribute("src", /bases\/mac-external\.webp/);

    await panel(page).getByRole("button", { name: /^choose a different folder than/ }).click();
    await expect(title(page)).toHaveText("Studio NAS");
    await expect(where(page)).toHaveText("Network drive · /Volumes/Studio NAS");
    await expect(stageImage(page)).toHaveAttribute("src", /bases\/mac-network\.webp/);
  });

  test("shows each skin on the drive, in the library and on the stage, and applies it there", async ({ page }) => {
    await pickDrive(page, "drive=external");
    // A card shows its skin drawn on the drive once it's ready, the plain drive until then.
    const art = tiles(page).first().locator(".tile-img:not(.is-under)");
    await expect(art).toHaveAttribute("src", /^data:image\/png/);
    await expect(tiles(page).first().locator(".tile-img.is-under")).toHaveCount(0);

    await tiles(page).first().locator(".tile-hit").click();
    await expect(panel(page).getByText(/^Trying on/)).toBeVisible();
    await expect(stageImage(page)).toHaveAttribute("src", await art.getAttribute("src") ?? "");
    await expect(status(page)).toHaveText("Nothing changes on disk until you apply.");

    await panel(page).getByRole("button", { name: "Apply skin" }).click();
    await expect(panel(page).getByText("Applied", { exact: true })).toBeVisible();
    await expect(stageImage(page)).toHaveAttribute("src", /^data:image\/png/);
    await panel(page).getByRole("button", { name: "Revert" }).click();
    await expect(page.getByText("Backup Disk has its default icon back")).toBeVisible();
    await expect(stageImage(page)).toHaveAttribute("src", /bases\/mac-external\.webp/);
  });

  test("with its subfolders, gives the drive its own icon and the folders on it the skin", async ({ page }) => {
    await pickDrive(page, "drive=external");
    await tiles(page).first().locator(".tile-hit").click();
    const include = panel(page).getByRole("switch", { name: /Include subfolders/ });
    await expect(include).toContainText("5 folders inside");
    await include.click();
    // The folders inside stack up behind it as folders, not as more drives.
    await expect(panel(page).locator(".stage-stack-img").first()).not.toHaveAttribute("src", /bases\/mac-external\.webp/);

    await panel(page).getByRole("button", { name: "Apply to 6 folders" }).click();
    await expect(panel(page).getByRole("button", { name: "Revert all 6" })).toBeVisible();
    // The drive wears the skin drawn for it.
    await expect(stageImage(page)).toHaveAttribute("src", /^data:image\/png/);

    // Revert takes it off the drive and the folders alike.
    await panel(page).getByRole("button", { name: "Revert all 6" }).click();
    await expect(panel(page).getByText("6 folders have the default icon back")).toBeVisible();
    await expect(stageImage(page)).toHaveAttribute("src", /bases\/mac-external\.webp/);
  });

  test("puts drive skins first while a drive is picked, and after the folder skins otherwise", async ({ page }) => {
    await openApp(page, { query: "driveskins=2&drive=external" });
    const drives = ["Plain mac external", "Plain mac removable"];
    await expect(tiles(page)).toHaveCount(10);
    expect((await names(page)).slice(-2)).toEqual(drives);

    await panel(page).getByRole("button", { name: /^choose a folder from/ }).click();
    await expect(title(page)).toHaveText("Backup Disk");
    await expect.poll(async () => (await names(page)).slice(0, 2)).toEqual(drives);
    // A finished drive is shown as it was drawn, not drawn again.
    await expect(tiles(page).first().locator(".tile-img")).toHaveAttribute("src", /bases\/mac-external\.webp$/);
    await expect(tiles(page).first().locator(".tile-img.is-under")).toHaveCount(0);

    // A folder puts them back after the folder skins.
    await panel(page).getByRole("button", { name: /^choose a different folder than/ }).click();
    await expect(title(page)).toHaveText("Projects");
    await expect.poll(async () => (await names(page)).slice(-2)).toEqual(drives);
  });

  test("tries a skin on a drive it can't change, and says why it can't be applied", async ({ page }) => {
    await pickDrive(page, "drive=startup,optical");
    await expect(title(page)).toHaveText("Macintosh HD");
    await expect(where(page)).toHaveText("Startup disk · /");
    await expect(status(page)).toHaveText("macOS keeps the startup disk sealed, so FolderSkin can't change its icon.");
    await tiles(page).first().locator(".tile-hit").click();
    await expect(panel(page).getByText(/^Trying on/)).toBeVisible();
    await expect(panel(page).getByRole("button", { name: "Apply skin" })).toBeDisabled();
    await expect(status(page)).toHaveText("macOS keeps the startup disk sealed, so FolderSkin can't change its icon.");
    // Nothing on it can be changed, so there's no run over its folders to offer.
    await expect(panel(page).getByRole("switch", { name: /Include subfolders/ })).toHaveCount(0);

    await panel(page).getByRole("button", { name: /^choose a different folder than/ }).click();
    await expect(title(page)).toHaveText("Holiday 2019");
    await expect(where(page)).toHaveText("Disc · /Volumes/Holiday 2019");
    await expect(status(page)).toHaveText("This drive is read-only, so FolderSkin can't change its icon.");
    await expect(panel(page).getByRole("button", { name: "Apply skin" })).toBeDisabled();
  });

  test("says so when a share's server turns the icon down", async ({ page }) => {
    await pickDrive(page, "os=linux&drive=network");
    await expect(title(page)).toHaveText("nas");
    await tiles(page).first().locator(".tile-hit").click();
    await panel(page).getByRole("button", { name: "Apply skin" }).click();
    await expect(status(page)).toHaveText("Couldn't apply the skin: the server didn't let FolderSkin change this share's icon");
    // Nothing was put on it, so it can be tried again.
    await expect(panel(page).getByRole("button", { name: "Apply skin" })).toBeEnabled();
  });

  test("starts a design on the drive that's chosen, with words that fit its face and read on it", async ({ page }) => {
    await pickDrive(page, "os=linux&drive=removable");
    await openView(page, /design your own/i);
    const chosen = page.getByRole("dialog").getByRole("group", { name: "The drive you chose" });
    await expect(chosen.locator(".cmp-card-name")).toHaveText("STICK");
    await expect(chosen.locator(".cmp-card-note")).toHaveText("USB stick");
    await chosen.getByRole("button").click();
    await expect(page.locator(".cmp-drive-pick")).toContainText("USB stick");

    await page.getByRole("button", { name: /^text$/i }).first().click();
    const words = async () => {
      const draft = await page.evaluate(() => sessionStorage.getItem("folderskin.composer.draft.v1") ?? "{}");
      const doc = (JSON.parse(draft) as { doc?: { layers: { kind: string; paint: { color: string }; size: number }[] } }).doc;
      return doc?.layers.find((l) => l.kind === "text") ?? null;
    };
    await expect.poll(async () => (await words())?.paint.color).toBe("#1b1f27");
    // The stick's label is narrow: the words are drawn small enough to fit on it.
    expect((await words())!.size).toBeLessThan(100);
  });

  test("is what a new AI chat paints for, among every shape the picker and @ list", async ({ page }) => {
    await pickDrive(page, "os=windows&drive=card");
    await openView(page, /generate with ai/i);
    const chip = page.locator(".shape-chip");
    await expect(chip).toHaveText("Windows SD card");
    await expect(page.locator(".studio-title")).toHaveText("What should your drive look like?");
    await chip.click();
    // The three folders, every drive each system shows, and the free icon.
    await expect(page.locator(".shape-option")).toHaveCount(3 + 9 + 6 + 10 + 1);
    await expect(page.getByRole("radio", { name: /^Windows SD card/ })).toHaveAttribute("aria-checked", "true");
    await expect(page.locator(".shape-make").getByText("Whole drive")).toBeVisible();
    await page.getByRole("radio", { name: /^Linux USB stick/ }).click();
    await expect(chip).toHaveText("Linux USB stick");

    await page.locator("textarea").first().fill("@hard");
    await expect(page.getByRole("option", { name: /^Linux USB hard disk/ })).toBeVisible();
  });

  test("names a Windows drive by its letter, and says the icon goes with the letter", async ({ page }) => {
    await pickDrive(page, "os=windows&drive=external,removable,startup");
    await expect(title(page)).toHaveText("Backup (E:)");
    await expect(where(page)).toHaveText("External drive · E:\\");

    // One with no name of its own is called by its kind, which isn't said twice.
    await panel(page).getByRole("button", { name: /^choose a different folder than/ }).click();
    await expect(title(page)).toHaveText("USB drive (F:)");
    await expect(where(page)).toHaveText("F:\\");
    await expect(stageImage(page)).toHaveAttribute("src", /bases\/windows-removable\.webp/);

    // Windows keeps a drive's icon in the registry, so even the system drive can have one.
    await panel(page).getByRole("button", { name: /^choose a different folder than/ }).click();
    await expect(title(page)).toHaveText("System drive (C:)");
    await expect(where(page)).toHaveText("C:\\");
    await tiles(page).first().locator(".tile-hit").click();
    await panel(page).getByRole("button", { name: "Apply skin" }).click();
    await expect(panel(page).getByText("Applied", { exact: true })).toBeVisible();
    await expect(status(page)).toHaveText("Explorer shows this icon for any drive that gets the letter C.");
  });
});

// The preview's Community has one pack of drives, Plain drives: six finished drives.
test.describe("packs of drives", () => {
  test("are marked in Community, and add drive skins that wait behind the folders' until a drive is picked", async ({ page }) => {
    await openApp(page, { query: "packs=0&drive=external" });
    await openView(page, /community/i);
    const card = page.locator(".pack:not(.is-placeholder)").filter({ has: page.locator(".pack-name", { hasText: /^Plain drives$/ }) });
    await expect(card.locator(".tag-chip.is-drives")).toHaveText("Drives");
    await card.getByRole("button", { name: "Add", exact: true }).click();
    await expect(card.getByRole("img", { name: "Added to your library" })).toBeVisible({ timeout: 10_000 });

    // Newest though they are, they come after the folder skins while no drive is picked.
    await openView(page, /all skins/i);
    await expect(tiles(page)).toHaveCount(14);
    expect((await names(page)).slice(-6).every((n) => n.startsWith("Plain "))).toBe(true);
    await panel(page).getByRole("button", { name: /^choose a folder from/ }).click();
    await expect(title(page)).toHaveText("Backup Disk");
    await expect.poll(async () => (await names(page)).slice(0, 6).every((n) => n.startsWith("Plain "))).toBe(true);
  });

  test("are saved as a folder for now, and never mixed with folders", async ({ page }) => {
    await openApp(page, { query: "driveskins=2" });
    await openView(page, /community/i);
    await page.getByRole("button", { name: "Share your skins" }).click();
    const dialog = page.getByRole("dialog", { name: "Share a pack" });
    const reason = dialog.locator(".modal-reason");
    await dialog.getByPlaceholder("Neon nights").fill("Backup drives");
    await dialog.getByRole("button", { name: "+ drive" }).click();
    await dialog.getByRole("textbox", { name: "the name your packs show" }).fill("sunny-otter");
    // Every skin of yours is ticked to start with, folders and drives.
    await expect(reason).toHaveText("A pack is all folders or all drives");
    await expect(dialog.getByRole("button", { name: "Save a folder" })).toBeDisabled();

    // The drives alone make a pack that is saved as a folder, and not sent.
    await dialog.getByRole("button", { name: "Clear" }).click();
    await dialog.getByRole("button", { name: /^Plain mac/ }).first().click();
    await dialog.getByRole("button", { name: /^Plain mac/ }).last().click();
    await expect(reason).toHaveText("For now, a pack of drives is saved as a folder");
    await expect(dialog.getByRole("button", { name: /send/i })).toBeDisabled();
    await expect(dialog.getByRole("button", { name: "Save a folder" })).toBeEnabled();
  });
});
