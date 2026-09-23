import { expect, test, type Page } from "@playwright/test";
import { openApp, openView } from "./app";

// One after another in one worker: each makes ten thousand packs, and five of them at once
// starved the rest of the suite (the composer's first load) and the timing below of the CPU.
test.describe.configure({ mode: "default" });

/** Opens Community with the sample packs and `packs` made-up ones, once the first answer is in. */
async function openCommunity(page: Page, packs = 10_000) {
  await openApp(page, { query: `packs=${packs}` });
  await openView(page, /community/i);
  await expect(page.locator(".community-count")).toHaveText(/\d packs?$/);
}

const cards = (page: Page) => page.locator(".pack:not(.is-placeholder)");
const search = (page: Page) => page.getByRole("searchbox", { name: /search packs/i });
const grid = (page: Page) => page.locator(".packs-grid");
const searches = (page: Page) => page.evaluate(() => (window as { mockCommunitySearches?: number }).mockCommunitySearches ?? 0);

test("typing searches ten thousand packs in under 300 ms, and only what's on screen is drawn", async ({ page }) => {
  await openCommunity(page);
  await expect(page.locator(".community-count")).toHaveText("10,004 packs");
  expect(await page.locator(".pack").count()).toBeLessThan(40);

  // One search first, as someone looking around would: the first also compiles the search code.
  await search(page).fill("lant");
  await expect(page.locator(".community-count")).toContainText("match “lant”");
  await search(page).fill("");
  await expect(page.locator(".community-count")).toHaveText("10,004 packs");

  // From the last key to the answer on screen, timed in the page itself.
  await page.evaluate(() => {
    const w = window as { typedAt?: number; shownAt?: number };
    document.addEventListener(
      "input",
      () => {
        w.typedAt = performance.now();
        w.shownAt = 0;
      },
      true,
    );
    const watch = () => {
      const shown = document.querySelector("[data-results-for]")?.getAttribute("data-results-for");
      const typed = document.querySelector<HTMLInputElement>('input[type="search"]')?.value.trim();
      if (w.typedAt && !w.shownAt && typed && shown === typed) w.shownAt = performance.now();
      requestAnimationFrame(watch);
    };
    watch();
  });
  await search(page).pressSequentially("harbour", { delay: 30 });
  await expect.poll(() => page.evaluate(() => (window as { shownAt?: number }).shownAt ?? 0)).toBeGreaterThan(0);
  const ms = await page.evaluate(() => {
    const w = window as { typedAt?: number; shownAt?: number };
    return (w.shownAt ?? 0) - (w.typedAt ?? 0);
  });
  test.info().annotations.push({ type: "last key to results", description: `${Math.round(ms)} ms` });
  expect(ms, `the answer took ${Math.round(ms)} ms after the last key`).toBeLessThan(300);
  await expect(page.locator(".community-count")).toContainText("match “harbour”");
  await expect(cards(page).first().locator(".pack-name")).toContainText(/harbour/i);

  // Far down the list, still only a screenful of cards, and the pages there arrive.
  await grid(page).evaluate((el) => (el.scrollTop = 40_000));
  await expect(cards(page).first()).toBeVisible();
  expect(await page.locator(".pack").count()).toBeLessThan(40);
});

test("a tag narrows the list, and the options sort it", async ({ page }) => {
  await openCommunity(page);
  const tab = page.getByRole("tab").nth(1);
  const tag = (await tab.locator(".count").evaluate((el) => el.parentElement!.firstChild!.textContent ?? "")).trim();
  await tab.click();
  await expect(tab).toHaveAttribute("aria-selected", "true");
  await expect(page.locator(".community-count")).toContainText(`tagged ${tag}`);
  const shown = await cards(page).count();
  expect(shown).toBeGreaterThan(0);
  for (let i = 0; i < Math.min(shown, 6); i++) {
    await expect(cards(page).nth(i).locator(".tag-chip", { hasText: new RegExp(`^${tag}$`, "i") })).toHaveCount(1);
  }

  await page.getByRole("button", { name: /sort and more tags/i }).click();
  await page.getByRole("radio", { name: "Name" }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: /sort and tags/i })).toHaveCount(0);
  await expect(page.getByRole("button", { name: /sort and more tags/i })).toHaveClass(/is-on/);
  await expect
    .poll(async () => {
      const names = (await cards(page).locator(".pack-name").allTextContents()).slice(0, 6).map((n) => n.toLowerCase());
      return names.length > 1 && names.every((n, i) => i === 0 || names[i - 1] <= n);
    })
    .toBe(true);
});

test("a pack opens to look through, and a skin that matched opens it at that skin", async ({ page }) => {
  await openCommunity(page);
  const first = cards(page).first();
  const name = (await first.getAttribute("aria-label"))!;
  await first.getByRole("button", { name: `view ${name}` }).click();
  const viewer = page.getByRole("dialog");
  await expect(viewer.getByRole("heading", { name })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(viewer).toHaveCount(0);

  await search(page).fill("maple");
  const hit = page.locator(".skin-hit").first();
  await expect(hit).toBeVisible();
  const skin = (await hit.locator(".skin-hit-name").textContent())!;
  const pack = (await hit.locator(".skin-hit-pack").textContent())!;
  await hit.click();
  await expect(viewer.getByRole("heading", { name: pack })).toBeVisible();
  await expect(viewer.locator(".pack-skin.is-focus")).toContainText(skin);
  await expect(viewer.locator(".pack-skin.is-focus")).toBeInViewport();
});

test("adding and updating a pack show how far they have got on its card", async ({ page }) => {
  await openCommunity(page, 0);
  const card = cards(page).filter({ has: page.locator(".pack-name", { hasText: /^Colours$/ }) });
  await card.getByRole("button", { name: "Add", exact: true }).click();
  const adding = card.getByRole("progressbar", { name: "Adding Colours" });
  await expect(adding).toBeVisible();
  await expect(adding).toHaveAttribute("aria-valuetext", /^(Downloading|Saving) \d+ of 8$/);
  await expect(card.getByRole("img", { name: "Added to your library" })).toBeVisible({ timeout: 10_000 });
  await expect(adding).toHaveCount(0);
  await expect(page.getByText(/Added 8 skins from Colours/)).toBeVisible();

  // The preview publishes a newer Colours as soon as it is added; Refresh finds it.
  await page.getByRole("button", { name: "refresh packs" }).click();
  await expect(page.getByText("1 of your packs has an update")).toBeVisible();
  await card.getByRole("button", { name: "Update", exact: true }).click();
  const updating = card.getByRole("progressbar", { name: "Updating Colours" });
  await expect(updating).toBeVisible();
  await expect(updating).toHaveAttribute("aria-valuetext", /^(Updating|(Downloading|Saving) \d+ of 8)$/);
  await expect(page.getByText("Updated Colours")).toBeVisible({ timeout: 10_000 });
  await expect(card.getByRole("button", { name: "Update", exact: true })).toHaveCount(0);
});

test("leaving Community and coming back finds it as it was, without searching again", async ({ page }) => {
  await openCommunity(page);
  await search(page).fill("neon");
  await expect(page.locator(".community-count")).toContainText("match “neon”");
  const tab = page.getByRole("tab").nth(1);
  await tab.click();
  await expect(page.locator(".community-count")).toContainText("tagged");
  const status = await page.locator(".community-count").textContent();
  await grid(page).evaluate((el) => (el.scrollTop = 1500));
  await expect.poll(() => grid(page).evaluate((el) => el.scrollTop)).toBeGreaterThan(1000);
  const scrolled = await grid(page).evaluate((el) => el.scrollTop);
  const before = await searches(page);

  await openView(page, /all skins/i);
  await expect(page.locator(".community")).toHaveCount(0);
  await openView(page, /community/i);

  await expect(search(page)).toHaveValue("neon");
  await expect(page.getByRole("tab").nth(1)).toHaveAttribute("aria-selected", "true");
  await expect(page.locator(".community-count")).toHaveText(status!);
  await expect.poll(() => grid(page).evaluate((el) => el.scrollTop)).toBe(scrolled);
  expect(await searches(page)).toBe(before);
});

test("without a connection it says so, and can try again", async ({ page }) => {
  await openApp(page, { query: "offline" });
  await openView(page, /community/i);
  await expect(page.getByText("The packs didn't load")).toBeVisible();
  await expect(page.getByText(/couldn't reach GitHub/i)).toBeVisible();
  await expect(page.getByRole("button", { name: "Try again" })).toBeVisible();
});
