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

/** The pack itself: a name, a tag, where the pictures came from, and the terms. */
async function fillPack(dialog: ReturnType<Page["getByRole"]>) {
  await dialog.getByPlaceholder("Neon nights").fill("Night prints");
  await dialog.getByRole("button", { name: "+ photo" }).click();
  await dialog.getByRole("combobox", { name: "The pictures" }).selectOption("own");
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
  await expect(dialog.getByRole("combobox", { name: "The pictures" })).toHaveCount(0);
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
  await dialog.getByRole("combobox", { name: "The pictures" }).selectOption("ai");
  await expect(dialog.getByText("Choose the name your packs show")).toBeVisible();
  await dialog.getByRole("textbox", { name: "the name your packs show" }).fill("ab");
  await expect(dialog.getByText("Choose the name your packs show")).toBeVisible();
  await dialog.getByRole("textbox", { name: "the name your packs show" }).fill("abc");
  await expect(dialog.getByText("Agree to the terms")).toBeVisible();
});
