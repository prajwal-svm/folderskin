import { expect, test, type Page } from "@playwright/test";
import { openApp, openView } from "./app";

/** Opens Community → Share your skins, and picks sharing without GitHub. */
async function shareWithoutGithub(page: Page, query = "") {
  await openApp(page, { query });
  await openView(page, /community/i);
  await page.getByRole("button", { name: "Share your skins" }).click();
  const dialog = page.getByRole("dialog", { name: "Share a pack" });
  await expect(dialog).toBeVisible();
  // GitHub stays the first choice, as it was.
  await expect(dialog.getByRole("radio", { name: "GitHub", exact: true })).toHaveAttribute("aria-checked", "true");
  await dialog.getByRole("radio", { name: "Without GitHub" }).click();
  return dialog;
}

/** Says where the pictures came from, in the app's own dropdown. */
async function pictures(dialog: ReturnType<Page["getByRole"]>, answer: string) {
  await dialog.getByRole("button", { name: /^the pictures:/ }).click();
  await dialog.page().getByRole("listbox", { name: "the pictures" }).getByRole("option", { name: answer }).click();
}

/** The pack itself: a name, a tag, where the pictures came from, and the terms. */
async function fillPack(dialog: ReturnType<Page["getByRole"]>) {
  await dialog.getByPlaceholder("Neon nights").fill("Night prints");
  await dialog.getByRole("button", { name: "+ photo" }).click();
  await pictures(dialog, "I made them myself");
  await dialog.getByText("I've read the pack terms and this pack follows them.").click();
}

test("a pack goes to the review queue without a GitHub account", async ({ page }) => {
  const dialog = await shareWithoutGithub(page);
  // What happens to it is said before anything is sent.
  await expect(dialog.getByText("Reviewed first.")).toBeVisible();
  await expect(dialog.getByText("Public once approved.")).toBeVisible();
  await expect(dialog.getByText(/a licence can't be taken back/)).toBeVisible();

  await fillPack(dialog);
  const name = dialog.getByRole("textbox", { name: "the name your packs show" });
  await name.fill("Sunny Otter");
  await expect(name).toHaveValue("Sunny-Otter");

  await dialog.getByRole("button", { name: "Verify and send" }).click();
  const verifying = page.getByRole("dialog", { name: "Verify this computer" });
  await expect(verifying.getByText(/has opened in your browser/)).toBeVisible();
  await expect(verifying.getByText("Sunny-Otter")).toBeVisible();

  // Once the check is passed in the browser, it carries on and sends by itself.
  const sent = page.getByRole("dialog", { name: "Your pack is waiting for review" });
  await expect(sent).toBeVisible({ timeout: 15_000 });
  await expect(sent.getByText("Night prints is in FolderSkin's review queue.")).toBeVisible();

  await sent.getByRole("button", { name: "See your submissions" }).click();
  const mine = page.getByRole("dialog", { name: "Your submissions" });
  const item = mine.getByRole("listitem").filter({ hasText: "Night prints" });
  await expect(item.getByText("Waiting for review")).toBeVisible();
  await expect(item.getByText(/8 pictures/)).toBeVisible();
});

test("a verified computer sends straight away, and sees why a pack was turned down", async ({ page }) => {
  const dialog = await shareWithoutGithub(page, "shared");
  await expect(dialog.getByText("sunny-otter")).toBeVisible();
  await expect(dialog.getByText("Verified on this computer")).toBeVisible();

  await dialog.getByRole("button", { name: "Your submissions" }).click();
  const mine = page.getByRole("dialog", { name: "Your submissions" });
  const turnedDown = mine.getByRole("listitem").filter({ hasText: "Neon cats" });
  await expect(turnedDown.getByText("Turned down")).toBeVisible();
  await expect(turnedDown.getByText("The pictures use someone else's logo, trade mark or characters.")).toBeVisible();
  await expect(turnedDown.getByRole("button", { name: /rule 13/i })).toBeVisible();
  await expect(turnedDown.getByRole("button", { name: "Withdraw" })).toHaveCount(0);

  // Withdrawing takes a second press, since an approved pack leaves the community for everyone.
  const approved = mine.getByRole("listitem").filter({ hasText: "Night prints" });
  await expect(approved.getByText("published as night-prints")).toBeVisible();
  await approved.getByRole("button", { name: "Withdraw" }).click();
  // It's in the community packs already, which only the maintainer can take it out of.
  await expect(approved.getByText(/leaves the community for everyone once FolderSkin's maintainer has taken it out/)).toBeVisible();
  await approved.getByRole("button", { name: "Withdraw it" }).click();
  await expect(approved.getByText("Withdrawn")).toBeVisible();
  await expect(approved.getByText(/maintainer has been told to take it out/)).toBeVisible();

  await mine.getByRole("button", { name: "Back to sharing" }).click();
  await fillPack(dialog);
  await dialog.getByRole("button", { name: "Send for review" }).click();
  await expect(page.getByRole("dialog", { name: "Your pack is waiting for review" })).toBeVisible({ timeout: 15_000 });
});

test("a build without the service says so instead of failing", async ({ page }) => {
  const dialog = await shareWithoutGithub(page, "noshare");
  await expect(dialog.getByText(/Sharing without GitHub isn't available yet/)).toBeVisible();
  // Nothing asks for what couldn't be sent anyway.
  await expect(dialog.getByRole("button", { name: /^the pictures:/ })).toHaveCount(0);
  await expect(dialog.getByText("Reviewed first.")).toHaveCount(0);
  await dialog.getByPlaceholder("Neon nights").fill("Night prints");
  await dialog.getByRole("button", { name: "+ photo" }).click();
  await dialog.getByText("I've read the pack terms and this pack follows them.").click();
  await expect(dialog.getByRole("button", { name: "Verify and send" })).toBeDisabled();
  await expect(dialog.getByText("Not available right now")).toBeVisible();
  // GitHub is still there, unchanged.
  await dialog.getByRole("radio", { name: "GitHub", exact: true }).click();
  await expect(dialog.getByRole("button", { name: "Connect to GitHub" })).toBeVisible();
  await expect(dialog.getByText("Connect to GitHub first")).toBeVisible();
});

test("without a connection to the service, it says that too", async ({ page }) => {
  const dialog = await shareWithoutGithub(page, "offline");
  await expect(dialog.getByText(/can't be reached right now/)).toBeVisible();
});

test("the send button says what is still missing", async ({ page }) => {
  const dialog = await shareWithoutGithub(page);
  await dialog.getByPlaceholder("Neon nights").fill("Night prints");
  await dialog.getByRole("button", { name: "+ photo" }).click();
  await expect(dialog.getByText("Say where the pictures came from")).toBeVisible();
  // Nothing is chosen for them: the field asks.
  await expect(dialog.getByRole("button", { name: /^the pictures:/ })).toHaveAccessibleName("the pictures: Where did they come from?");
  await pictures(dialog, "I made them with an AI model");
  await expect(dialog.getByText("Choose the name your packs show")).toBeVisible();
  await dialog.getByRole("textbox", { name: "the name your packs show" }).fill("ab");
  await expect(dialog.getByText("Choose the name your packs show")).toBeVisible();
  await dialog.getByRole("textbox", { name: "the name your packs show" }).fill("abc");
  await expect(dialog.getByText("Agree to the terms")).toBeVisible();
});

test("the skins shown narrow to one tag, and name the pack after it", async ({ page }) => {
  await openApp(page);
  await openView(page, /community/i);
  await page.getByRole("button", { name: "Share your skins" }).click();
  const dialog = page.getByRole("dialog", { name: "Share a pack" });
  const shown = dialog.getByRole("button", { name: /^which skins to show:/ });
  await expect(shown).toHaveAccessibleName("which skins to show: All of yours (8)");
  await expect(dialog.locator(".share-pick-one")).toHaveCount(8);
  await shown.click();
  await page.getByRole("listbox", { name: "which skins to show" }).getByRole("option", { name: "Tagged painting (3)" }).click();
  await expect(dialog.locator(".share-pick-one")).toHaveCount(3);
  await expect(dialog.getByPlaceholder("Neon nights")).toHaveValue("Painting");
});

test.describe("licence profiles", () => {
  const PROFILES_KEY = "folderskin.sharing.profiles";
  const profiles = {
    list: [
      { id: "a", name: "Personal", author: "", license: "CC0-1.0" },
      { id: "b", name: "For work", author: "acme-studio", license: "CC-BY-4.0" },
    ],
    defaultId: "a",
  };
  const kept = (page: Page) => page.evaluate((key) => JSON.parse(localStorage.getItem(key) ?? "null"), PROFILES_KEY);

  /** Opens the share dialog with the two profiles above kept in Settings. */
  async function openShare(page: Page, query = "") {
    await page.addInitScript(([key, value]) => localStorage.setItem(key, value), [PROFILES_KEY, JSON.stringify(profiles)]);
    await openApp(page, { query });
    await openView(page, /community/i);
    await page.getByRole("button", { name: "Share your skins" }).click();
    const dialog = page.getByRole("dialog", { name: "Share a pack" });
    await expect(dialog).toBeVisible();
    return dialog;
  }

  test("give the pack its credit and licence, and a licence changed here is for this pack only", async ({ page }) => {
    const dialog = await openShare(page);
    const profile = dialog.getByRole("button", { name: /^profile:/ });
    const licence = dialog.getByRole("button", { name: /^licence:/ });
    // The default first.
    await expect(profile).toHaveAccessibleName("profile: Personal · CC0");
    await expect(licence).toHaveAccessibleName(/^licence: CC0:/);

    await profile.click();
    const options = page.getByRole("listbox", { name: "profile" }).getByRole("option");
    await expect(options).toHaveText(["Personal · CC0", "For work · credited to acme-studio · CC BY 4.0"]);
    await options.filter({ hasText: "For work" }).click();
    await expect(licence).toHaveAccessibleName(/^licence: CC BY 4.0:/);

    await licence.click();
    await page.getByRole("listbox", { name: "licence" }).getByRole("option", { name: /^MIT/ }).click();
    await expect(dialog.getByText("For this pack only: the For work profile stays CC BY 4.0.")).toBeVisible();

    // Without GitHub, the name it credits is the one typed for a computer not yet verified.
    await dialog.getByRole("radio", { name: "Without GitHub" }).click();
    await expect(dialog.getByRole("textbox", { name: "the name your packs show" })).toHaveValue("acme-studio");
    expect(await kept(page)).toEqual(profiles);
  });

  test("say when the pack is credited to someone else, and publishing changes none of them", async ({ page }) => {
    const dialog = await openShare(page, "shared");
    await dialog.getByRole("button", { name: /^profile:/ }).click();
    await page.getByRole("listbox", { name: "profile" }).getByRole("option", { name: /For work/ }).click();
    await dialog.getByRole("button", { name: /^licence:/ }).click();
    await page.getByRole("listbox", { name: "licence" }).getByRole("option", { name: /^MIT/ }).click();

    // A verified computer sends under the name it was verified as.
    await dialog.getByRole("radio", { name: "Without GitHub" }).click();
    await expect(dialog.getByText("Verified on this computer")).toBeVisible();
    await expect(dialog.getByText(/The For work profile credits acme-studio\. Packs sent from this computer are credited to the name it was verified under\./)).toBeVisible();

    // Through GitHub, the pack is credited to the account connected.
    await dialog.getByRole("radio", { name: "GitHub", exact: true }).click();
    await dialog.getByRole("button", { name: "Connect to GitHub" }).click();
    await expect(dialog.getByText(/The For work profile credits acme-studio\. Through GitHub, packs are credited to the account connected here\./)).toBeVisible({ timeout: 10_000 });
    await expect(dialog.getByText("octocat")).toBeVisible();
    await dialog.getByPlaceholder("Neon nights").fill("Night prints");
    await dialog.getByRole("button", { name: "+ photo" }).click();
    await dialog.getByText("I've read the pack terms and this pack follows them.").click();
    await dialog.getByRole("button", { name: "Publish" }).click();
    await expect(page.getByRole("dialog", { name: "Your pack is on its way" })).toBeVisible({ timeout: 20_000 });

    // The default profile, which credited no one, now credits that account; nothing else changed.
    expect(await kept(page)).toEqual({ ...profiles, list: [{ ...profiles.list[0], author: "octocat" }, profiles.list[1]] });
  });
});
