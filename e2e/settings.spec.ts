import { expect, test, type Page } from "@playwright/test";
import { openApp } from "./app";

const dialog = (page: Page) => page.getByRole("dialog", { name: "Settings" });

async function openSettings(page: Page) {
  await page.getByRole("button", { name: /^settings$/i }).first().click();
  await expect(dialog(page)).toBeVisible();
}

const html = (page: Page, attr: string) => page.evaluate((a) => document.documentElement.getAttribute(a), attr);

test.describe("settings", () => {
  test("is its pages down the side and the chosen one beside them, moved through with the arrow keys", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    const tabs = dialog(page).getByRole("tab");
    await expect(tabs).toHaveText(["General", "AI", "Sharing", "About"]);
    await expect(dialog(page).getByRole("tab", { name: "General" })).toHaveAttribute("aria-selected", "true");
    await expect(dialog(page).getByRole("tab", { name: "General" })).toBeFocused();
    await expect(dialog(page).getByRole("heading", { name: "Appearance" })).toBeVisible();
    await page.keyboard.press("ArrowDown");
    await expect(dialog(page).getByRole("tab", { name: "AI" })).toHaveAttribute("aria-selected", "true");
    await expect(dialog(page).getByRole("heading", { name: "Where pictures are made" })).toBeVisible();
    await page.keyboard.press("End");
    await expect(dialog(page).getByRole("heading", { name: "Updates" })).toBeVisible();
    // Nothing in it uses the system's tooltips.
    const titled = await dialog(page).evaluate((d) => [...d.querySelectorAll("[title], svg title")].length);
    expect(titled).toBe(0);
  });

  test("theme, accent colour and motion change the whole app, and are kept", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    await dialog(page).getByRole("radiogroup", { name: "theme" }).getByRole("radio", { name: "Dark" }).click();
    await expect.poll(() => html(page, "data-theme")).toBe("dark");
    await dialog(page).getByRole("radio", { name: "Purple" }).click();
    await expect.poll(() => html(page, "data-accent")).toBe("purple");
    // The accent is the app's: the primary buttons take it.
    const accent = await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--accent").trim());
    expect(accent).toBe("#8b5cf6");
    await dialog(page).getByRole("radiogroup", { name: "motion" }).getByRole("radio", { name: "Reduced" }).click();
    await expect.poll(() => html(page, "data-motion")).toBe("reduced");

    await page.reload();
    await expect(page.getByRole("button", { name: /all skins/i }).first()).toBeVisible();
    expect(await html(page, "data-theme")).toBe("dark");
    expect(await html(page, "data-accent")).toBe("purple");
    expect(await html(page, "data-motion")).toBe("reduced");

    // Black and white is black on the light window and white on the dark one, with dark words on it.
    await openSettings(page);
    await dialog(page).getByRole("radio", { name: "Black and white" }).click();
    const mono = () =>
      page.evaluate(() => {
        const css = getComputedStyle(document.documentElement);
        return [css.getPropertyValue("--accent").trim(), css.getPropertyValue("--on-accent").trim()];
      });
    await expect.poll(mono).toEqual(["#f2eeed", "#1f1c1c"]);
    await dialog(page).getByRole("radiogroup", { name: "theme" }).getByRole("radio", { name: "Light" }).click();
    await expect.poll(mono).toEqual(["#1d1d1f", "#ffffff"]);

    // Back to the defaults: no attributes left behind.
    await dialog(page).getByRole("radio", { name: "Blue" }).click();
    await dialog(page).getByRole("radiogroup", { name: "motion" }).getByRole("radio", { name: "System" }).click();
    await expect.poll(() => html(page, "data-accent")).toBeNull();
    await expect.poll(() => html(page, "data-motion")).toBeNull();
  });

  test("Motion: Reduced keeps the icons and the library's folders still", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    // The globe on the Sharing tab draws itself again when pointed at, unless motion is reduced.
    const globe = dialog(page).getByRole("tab", { name: "Sharing" }).locator("svg");
    const drawn = async () => {
      // Its way back from being drawn takes a little over a second.
      await page.mouse.move(0, 0);
      await page.waitForTimeout(1500);
      const before = await globe.evaluate((s) => s.outerHTML);
      await dialog(page).getByRole("tab", { name: "Sharing" }).hover();
      await page.waitForTimeout(250);
      return before !== (await globe.evaluate((s) => s.outerHTML));
    };
    expect(await drawn()).toBe(true);
    await dialog(page).getByRole("radiogroup", { name: "motion" }).getByRole("radio", { name: "Reduced" }).click();
    expect(await drawn()).toBe(false);

    await page.keyboard.press("Escape");
    await expect(dialog(page)).toBeHidden();
    const tile = page.locator(".tile-hit").first();
    const box = (await tile.boundingBox())!;
    await page.mouse.move(box.x + box.width * 0.85, box.y + box.height * 0.2);
    await page.mouse.move(box.x + box.width * 0.9, box.y + box.height * 0.15);
    await page.waitForTimeout(300);
    expect(await tile.locator(".tile-art").evaluate((el) => (el as HTMLElement).style.transform)).toBe("");
  });

  test("the accent colours are chosen with the arrow keys too", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    await dialog(page).getByRole("radio", { name: "Blue" }).focus();
    await page.keyboard.press("ArrowRight");
    await expect(dialog(page).getByRole("radio", { name: "Purple" })).toHaveAttribute("aria-checked", "true");
    await expect(dialog(page).getByRole("radio", { name: "Purple" })).toBeFocused();
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowLeft");
    await expect(dialog(page).getByRole("radio", { name: "Black and white" })).toHaveAttribute("aria-checked", "true");
  });

  test("Tab stops once at each group, on its chosen one, and the arrow keys move within it", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    await expect(dialog(page).getByRole("tab", { name: "General" })).toBeFocused();
    const theme = dialog(page).getByRole("radiogroup", { name: "theme" });
    await page.keyboard.press("Tab");
    await expect(theme.getByRole("radio", { checked: true })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(dialog(page).getByRole("radio", { name: "Blue" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(theme.getByRole("radio", { checked: true })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(dialog(page).getByRole("tab", { name: "General" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(dialog(page).getByLabel("search settings")).toBeFocused();
  });

  test("Escape in an open list closes the list and leaves the dialog and what's typed in it", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    await dialog(page).getByRole("tab", { name: "Sharing" }).click();
    await dialog(page).getByRole("button", { name: "Add a profile" }).click();
    const form = dialog(page).getByRole("form", { name: "new profile" });
    await form.getByLabel("Name").fill("Typed but not saved");
    await form.getByRole("button", { name: "licence" }).click();
    await expect(page.getByRole("listbox")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await expect(dialog(page)).toBeVisible();
    await expect(form.getByLabel("Name")).toHaveValue("Typed but not saved");
    await expect(form.getByRole("button", { name: "licence" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(dialog(page)).toHaveCount(0);
  });

  test("Escape in the search empties it first, and closes the dialog after", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    const search = dialog(page).getByLabel("search settings");
    await search.fill("motion");
    await expect(dialog(page).getByRole("tab")).toHaveText(["General"]);
    await page.keyboard.press("Escape");
    await expect(dialog(page)).toBeVisible();
    await expect(search).toHaveValue("");
    await expect(dialog(page).getByRole("tab")).toHaveCount(4);
    await expect(search).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(dialog(page)).toHaveCount(0);
  });

  test("folds the sidebar to its icons and back", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    const sidebar = dialog(page).getByRole("radiogroup", { name: "sidebar" });
    await sidebar.getByRole("radio", { name: "Icons only" }).click();
    await expect(page.locator(".app")).toHaveClass(/is-rail/);
    await sidebar.getByRole("radio", { name: "Full" }).click();
    await expect(page.locator(".app")).not.toHaveClass(/is-rail/);
  });

  test("search keeps the pages that have it and lights up the setting", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    const search = dialog(page).getByLabel("search settings");
    await search.fill("licence");
    await expect(dialog(page).getByRole("tab")).toHaveText(["Sharing"]);
    await expect(dialog(page).getByRole("heading", { name: "Licence profiles" })).toBeVisible();
    await search.fill("accent");
    await expect(dialog(page).getByRole("tab")).toHaveText(["General"]);
    await expect(dialog(page).locator(".set-row.is-match")).toHaveText(/Accent colour/);
    // Nothing found: the page gives way to saying so, and the panel isn't named after a tab that's gone.
    await search.fill("zzzz");
    const panel = dialog(page).getByRole("tabpanel");
    await expect(panel).toHaveText(/No setting matches “zzzz”/);
    await expect(panel).toHaveAccessibleName("settings");
    await expect(dialog(page).getByRole("heading", { name: "Appearance" })).toBeHidden();
    await search.fill("");
    await expect(dialog(page).getByRole("tab")).toHaveCount(4);
    await expect(dialog(page).getByRole("heading", { name: "Appearance" })).toBeVisible();
  });

  test("search finds a page's headings and a setting's words in any order", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    const search = dialog(page).getByLabel("search settings");
    const lit = dialog(page).locator(".is-match");
    for (const [words, tab, found] of [
      ["licence profiles", "Sharing", "Licence profiles"],
      ["dark mode", "General", "Theme"],
      ["accent color", "General", "Accent colour"],
      ["your skins", "General", "Your skins"],
      ["reduce motion", "General", "Motion"],
      ["where pictures are made", "AI", "Where pictures are made"],
      ["openai", "AI", "Where pictures are made"],
    ]) {
      await search.fill(words);
      await expect(dialog(page).getByRole("tab", { name: tab })).toHaveAttribute("aria-selected", "true");
      await expect(lit.first()).toContainText(found);
    }
    // One letter keeps the pages that have it, but lights nothing: it's in nearly everything.
    await search.fill("e");
    await expect(dialog(page).getByRole("tab")).toHaveCount(4);
    await expect(lit).toHaveCount(0);
  });

  test("licence profiles are added, made the default, checked, deleted and brought back", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    await dialog(page).getByRole("tab", { name: "Sharing" }).click();
    const rows = dialog(page).locator(".set-profile");
    await expect(rows).toHaveCount(1);
    await expect(rows.first()).toContainText("Personal");
    await expect(rows.first()).toContainText("Default");

    await dialog(page).getByRole("button", { name: "Add a profile" }).click();
    const form = dialog(page).getByRole("form", { name: "new profile" });
    await form.getByLabel("Name").fill("personal");
    await form.getByRole("button", { name: "Add profile" }).click();
    // The problem shows under the field it's about, which is marked and has the focus.
    await expect(form.getByRole("alert")).toHaveText(/already/);
    await expect(form.getByLabel("Name")).toHaveAttribute("aria-invalid", "true");
    await expect(form.getByLabel("Name")).toBeFocused();
    await expect(form.getByLabel("Name")).toHaveAccessibleDescription(/already/);
    await form.getByLabel("Name").fill("For work");
    await form.getByLabel("Credited to").fill("acme studio");
    await form.getByRole("button", { name: "Add profile" }).click();
    await expect(form.getByRole("alert")).toHaveText(/GitHub user name/);
    await expect(form.getByLabel("Credited to")).toHaveAttribute("aria-invalid", "true");
    await expect(form.getByLabel("Credited to")).toBeFocused();
    await expect(form.getByLabel("Name")).not.toHaveAttribute("aria-invalid");
    await form.getByLabel("Credited to").fill("acme-studio");
    await form.getByRole("button", { name: "licence" }).click();
    await page.getByRole("option", { name: /CC BY 4.0/ }).click();
    await form.getByRole("button", { name: "Add profile" }).click();
    await expect(rows).toHaveCount(2);
    await expect(rows.nth(1)).toContainText("Credited to acme-studio · CC BY 4.0");

    await rows.nth(1).hover();
    await rows.nth(1).getByRole("button", { name: "make For work the default" }).click();
    await expect(rows.nth(1)).toContainText("Default");
    await expect(rows.first()).not.toContainText("Default");

    await rows.first().hover();
    await rows.first().getByRole("button", { name: "delete Personal" }).click();
    await expect(rows).toHaveCount(1);
    await dialog(page).getByRole("button", { name: "Undo" }).click();
    await expect(rows).toHaveCount(2);

    // Kept for next time.
    await page.reload();
    await expect(page.getByRole("button", { name: /all skins/i }).first()).toBeVisible();
    await openSettings(page);
    await dialog(page).getByRole("tab", { name: "Sharing" }).click();
    await expect(rows).toHaveCount(2);
    await expect(rows.nth(1)).toContainText("Default");
  });

  test("the keyboard's place shows on the chosen page and the chosen option", async ({ page }) => {
    // Still, so every colour is read where it ends up rather than on its way.
    await page.addInitScript(() => localStorage.setItem("folderskin.prefs", JSON.stringify({ accent: "blue", motion: "reduced" })));
    await openApp(page);
    await openSettings(page);
    const look = async (el: ReturnType<typeof dialog>) => {
      await page.waitForTimeout(100);
      return el.evaluate((e) => ((s) => `${s.backgroundColor} ${s.color} ${s.borderColor}`)(getComputedStyle(e)));
    };
    const away = () => dialog(page).getByLabel("search settings").click();
    // Onto another page and back with the keyboard, then away with the pointer.
    const general = dialog(page).getByRole("tab", { name: "General" });
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowUp");
    await expect(general).toBeFocused();
    const generalFocused = await look(general);
    await away();
    await expect.poll(() => look(general)).not.toBe(generalFocused);
    // The same for the chosen one of a few, chosen with the arrow keys.
    const sidebar = dialog(page).getByRole("radiogroup", { name: "sidebar" });
    await sidebar.getByRole("radio", { name: "Full" }).focus();
    await page.keyboard.press("ArrowRight");
    const icons = sidebar.getByRole("radio", { name: "Icons only" });
    await expect(icons).toBeFocused();
    await expect(icons).toHaveAttribute("aria-checked", "true");
    const iconsFocused = await look(icons);
    await away();
    await expect.poll(() => look(icons)).not.toBe(iconsFocused);
  });

  test("the plain folder stands for where the skins are kept", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    const row = dialog(page).locator(".set-row").filter({ hasText: /\d+ skins?/ });
    await expect(row.locator("img.set-row-folder")).toHaveAttribute("src", /.+/);
    await expect(row.locator(".set-row-lead")).toHaveCount(0);
  });

  test("the focus stays with the profiles as they're added, changed, deleted and brought back", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    await dialog(page).getByRole("tab", { name: "Sharing" }).click();
    const add = dialog(page).getByRole("button", { name: "Add a profile" });
    const name = dialog(page).getByRole("form").getByLabel("Name");

    await add.focus();
    await page.keyboard.press("Enter");
    await expect(name).toBeFocused();
    await expect(name).toHaveAttribute("maxlength", "40");
    await dialog(page).getByRole("button", { name: "Cancel" }).focus();
    await page.keyboard.press("Enter");
    await expect(add).toBeFocused();

    await page.keyboard.press("Enter");
    await page.keyboard.type("Work");
    await page.keyboard.press("Enter");
    await expect(dialog(page).locator(".set-profile")).toHaveCount(2);
    await expect(add).toBeFocused();

    const change = dialog(page).getByRole("button", { name: "change Work" });
    await change.focus();
    await page.keyboard.press("Enter");
    await expect(name).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(change).toBeFocused();

    // The note that it's gone is read out: its place is there before it comes.
    const status = dialog(page).getByRole("status");
    await expect(status).toHaveText("");
    await dialog(page).getByRole("button", { name: "delete Work" }).focus();
    await page.keyboard.press("Enter");
    await expect(status).toHaveText(/Deleted Work\./);
    await expect(dialog(page).getByRole("button", { name: "Undo" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(change).toBeFocused();
  });

  test("a new profile isn't lost to a list that filled up while it was being written", async ({ page }) => {
    await page.addInitScript(() => {
      const list = Array.from({ length: 12 }, (_, i) => ({ id: `p${i}`, name: `P${i}`, author: "", license: "CC0-1.0" }));
      if (!sessionStorage.getItem("seeded")) {
        localStorage.setItem("folderskin.sharing.profiles", JSON.stringify({ list, defaultId: "p0" }));
        sessionStorage.setItem("seeded", "1");
      }
    });
    await openApp(page);
    await openSettings(page);
    await dialog(page).getByRole("tab", { name: "Sharing" }).click();
    const rows = dialog(page).locator(".set-profile");
    await expect(rows).toHaveCount(12);
    await rows.last().hover();
    await rows.last().getByRole("button", { name: "delete P11" }).click();
    await dialog(page).getByRole("button", { name: "Add a profile" }).click();
    const form = dialog(page).getByRole("form", { name: "new profile" });
    // Below a long list, the whole form comes into view, its buttons too.
    await expect(form.getByRole("button", { name: "Add profile" })).toBeInViewport({ ratio: 1 });
    await form.getByLabel("Name").fill("Brand new");
    await dialog(page).getByRole("button", { name: "Undo" }).click();
    await expect(rows).toHaveCount(12);
    await form.getByRole("button", { name: "Add profile" }).click();
    await expect(form.getByRole("alert")).toHaveText(/12 profiles at most/);
    await expect(form).toBeVisible();
    await expect(form.getByLabel("Name")).toHaveValue("Brand new");
  });

  test("searching doesn't leave a profile half changed", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    await dialog(page).getByRole("tab", { name: "Sharing" }).click();
    await dialog(page).locator(".set-profile").first().hover();
    await dialog(page).getByRole("button", { name: "change Personal" }).click();
    const form = dialog(page).getByRole("form", { name: "change Personal" });
    await form.getByLabel("Name").fill("Personal (edited)");
    const search = dialog(page).getByLabel("search settings");
    await search.fill("theme");
    await expect(dialog(page).getByRole("tab")).toHaveText(["General"]);
    // The page stays, named for itself, with the profile still open.
    await expect(dialog(page).getByRole("tabpanel", { name: "Sharing" })).toBeVisible();
    await expect(form.getByLabel("Name")).toHaveValue("Personal (edited)");
    await search.fill("zzzz");
    await search.fill("");
    await expect(form.getByLabel("Name")).toHaveValue("Personal (edited)");
    await form.getByRole("button", { name: "Save" }).click();
    await expect(dialog(page).locator(".set-profile").first()).toContainText("Personal (edited)");
  });

  test("GitHub connects in its own section, keeps the focus, and credits the default profile", async ({ page }) => {
    await openApp(page);
    await openSettings(page);
    await dialog(page).getByRole("tab", { name: "Sharing" }).click();
    await dialog(page).getByRole("button", { name: "Connect", exact: true }).focus();
    await page.keyboard.press("Enter");
    await expect(dialog(page).getByRole("button", { name: "Open GitHub" })).toBeFocused();
    // The profiles stay below while it waits.
    await expect(dialog(page).getByRole("heading", { name: "Licence profiles" })).toBeVisible();
    const disconnect = dialog(page).getByRole("button", { name: "Disconnect" });
    await expect(disconnect).toBeFocused({ timeout: 10_000 });
    const rows = dialog(page).locator(".set-profile");
    await expect(rows.first()).toContainText("Credited to octocat");

    // A new default with no one to credit takes the name when Sharing finds the account again.
    await dialog(page).getByRole("button", { name: "Add a profile" }).click();
    const form = dialog(page).getByRole("form", { name: "new profile" });
    await form.getByLabel("Name").fill("Work");
    await form.getByLabel("Credited to").fill("");
    await form.getByRole("button", { name: "Add profile" }).click();
    await rows.nth(1).hover();
    await rows.nth(1).getByRole("button", { name: "make Work the default" }).click();
    await expect(rows.nth(1)).toContainText("No one to credit yet");
    await page.keyboard.press("Escape");
    await openSettings(page);
    await dialog(page).getByRole("tab", { name: "Sharing" }).click();
    await expect(rows.nth(1)).toContainText("Credited to octocat");

    await disconnect.focus();
    await page.keyboard.press("Enter");
    await expect(dialog(page).getByRole("button", { name: "Connect", exact: true })).toBeFocused();
  });

  test("every accent keeps the words on its buttons readable", async ({ page }) => {
    await openApp(page);
    const ratios = await page.evaluate(() => {
      const lum = (rgb: string) => {
        const [r, g, b] = (rgb.match(/[\d.]+/g) ?? []).slice(0, 3).map((v) => {
          const c = Number(v) / 255;
          return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
        });
        return 0.2126 * r + 0.7152 * g + 0.0722 * b;
      };
      const ratio = (a: string, b: string) => {
        const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
        return (x + 0.05) / (y + 0.05);
      };
      const probe = document.createElement("span");
      document.body.append(probe);
      const colour = (v: string) => {
        probe.style.color = `var(${v})`;
        return getComputedStyle(probe).color;
      };
      const root = document.documentElement;
      const out: Record<string, number> = {};
      for (const theme of ["light", "dark"])
        for (const accent of ["blue", "purple", "pink", "orange", "green", "mono"]) {
          root.dataset.theme = theme;
          root.dataset.accent = accent;
          for (const bg of ["--accent", "--accent-hover"]) out[`${theme} ${accent} ${bg}`] = ratio(colour("--on-accent"), colour(bg));
        }
      probe.remove();
      return out;
    });
    for (const [where, r] of Object.entries(ratios)) expect(r, where).toBeGreaterThanOrEqual(3);
  });
});
