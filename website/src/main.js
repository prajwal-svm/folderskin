import {
  API_URL,
  RELEASES_URL,
  detectSystem,
  recommendedChoice,
  fetchLatestRelease,
  selectInstaller,
  formatSize,
} from "./downloads.js";
import snapshot from "./release-snapshot.json";
import {
  applyTheme,
  loadThemePref,
  resolveTheme,
  saveThemePref,
} from "../../src/state/theme.ts";

const $ = (selector) => document.querySelector(selector);
const themeButton = $(".theme-toggle");
const themeSelect = $("#theme-preference");
const mediaQuery = matchMedia("(prefers-color-scheme: dark)");
let themePreference = loadThemePref();
function renderTheme() {
  const theme = resolveTheme(themePreference);
  applyTheme(theme);
  $("#app-source").media = theme === "dark" ? "all" : "not all";
  const label = `Switch to ${theme === "dark" ? "light" : "dark"} mode`;
  themeButton.setAttribute("aria-label", label);
  themeButton.title = label;
  themeButton
    .querySelector("use")
    .setAttribute("href", theme === "dark" ? "#i-sun" : "#i-moon");
  themeSelect.value = themePreference;
  $('meta[name="theme-color"]').content =
    theme === "dark" ? "#1f1c1c" : "#e9edf2";
}
function setTheme(preference) {
  themePreference = preference;
  saveThemePref(preference);
  renderTheme();
}
renderTheme();
themeButton.addEventListener("click", () =>
  setTheme(
    document.documentElement.dataset.theme === "dark" ? "light" : "dark",
  ),
);
themeSelect.addEventListener("change", () => setTheme(themeSelect.value));
mediaQuery.addEventListener("change", () => {
  if (themePreference === "system") renderTheme();
});
window.addEventListener("storage", (event) => {
  if (event.key === "folderskin.theme" || event.key === null) {
    themePreference = loadThemePref();
    renderTheme();
  }
});

if (Number.isInteger(snapshot.stars))
  $("[data-stars]").textContent = new Intl.NumberFormat("en", {
    notation: "compact",
  }).format(snapshot.stars);

const labels = { macos: "macOS", windows: "Windows", linux: "Linux" };
let system = detectSystem(navigator);
let release = snapshot.release;
let busy = false;
let live = false;
let pendingRelease;
const status = $("#download-status");

function getLatest() {
  if (!pendingRelease)
    pendingRelease = fetchLatestRelease().finally(() => {
      pendingRelease = null;
    });
  return pendingRelease;
}
function installerFor(os, choice) {
  const [architecture, format] = choice.split(":");
  return selectInstaller(release, os, architecture, format);
}
function renderDownloads() {
  $("[data-release]").textContent = `${release.tag} · Free & open source`;
  $("[data-version-link]").firstChild.textContent =
    `${release.tag} · Release notes `;
  for (const select of document.querySelectorAll("[data-choice]")) {
    const os = select.dataset.choice;
    const available = Array.from(select.options).filter((option) => {
      option.disabled = !installerFor(os, option.value);
      option.hidden = option.disabled;
      return !option.disabled;
    });
    if (!available.some((option) => option.value === select.value))
      select.value = available[0]?.value || "";
    const hasChoices = available.length > 1;
    select.closest("[data-choice-control]").hidden = !hasChoices;
    const summary = $(`[data-installer-summary="${os}"]`);
    summary.hidden = hasChoices;
    summary.textContent = available[0]?.textContent || "No installer available";
    const installer = installerFor(os, select.value);
    const link = $(`[data-installer="${os}"]`);
    link.href = installer?.browser_download_url || RELEASES_URL;
    $(`[data-asset-detail="${os}"]`).textContent = installer
      ? `${release.tag} · ${formatSize(installer.size)}`
      : "See available installers on GitHub";
  }
  const choice = recommendedChoice(system);
  const installer = choice && installerFor(system.os, choice);
  const primary = $("[data-primary-download]");
  primary.href = installer?.browser_download_url || "#downloads";
  $("[data-download-label]").textContent = installer
    ? `Download for ${labels[system.os]}`
    : "Choose your download";
  primary
    .querySelector("use")
    .setAttribute(
      "href",
      installer
        ? `#i-${{ macos: "apple", windows: "windows", linux: "linux" }[system.os]}`
        : "#i-download",
    );
  const platformNote =
    system.os === "macos"
      ? "Apple silicon & Intel"
      : system.os === "windows" && system.architecture === "arm64"
        ? "x64 installer · Runs with Windows emulation"
        : system.os === "linux"
          ? system.architecture === "arm64"
            ? "ARM64 · AppImage"
            : "x86_64 · AppImage"
          : "Windows x64";
  $("[data-download-caption]").textContent = installer
    ? `${platformNote} · ${formatSize(installer.size)} · Free`
    : system.mobile
      ? "A desktop app for macOS, Windows, and Linux"
      : "Free for macOS, Windows, and Linux";
  for (const card of document.querySelectorAll("[data-platform]")) {
    const recommended =
      Boolean(installer) && card.dataset.platform === system.os;
    card.classList.toggle("recommended", recommended);
    card.querySelector(".recommendation").hidden = !recommended;
  }
}
renderDownloads();
for (const select of document.querySelectorAll("[data-choice]"))
  select.addEventListener("change", renderDownloads);

async function download(event, os, choice) {
  if (
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey ||
    event.altKey ||
    event.button !== 0 ||
    !os ||
    !choice
  )
    return;
  event.preventDefault();
  if (busy) return;
  busy = true;
  const link = event.currentTarget;
  link.setAttribute("aria-busy", "true");
  status.textContent = "Finding the latest installer…";
  try {
    release = await getLatest();
    live = true;
    renderDownloads();
    const asset = installerFor(os, choice);
    if (!asset) throw new Error("This installer is not available.");
    status.textContent = `Downloading FolderSkin ${release.tag} for ${labels[os]}.`;
    window.location.assign(asset.browser_download_url);
  } catch {
    // Never silently send an older snapshot when the latest release cannot be resolved.
    status.replaceChildren(
      document.createTextNode("The latest installer could not be reached. "),
    );
    const fallback = document.createElement("a");
    fallback.href = RELEASES_URL;
    fallback.textContent = "Get it from GitHub releases ↗";
    fallback.className = "text-link";
    status.append(fallback);
    status.scrollIntoView({ behavior: "auto", block: "center" });
  } finally {
    busy = false;
    link.removeAttribute("aria-busy");
  }
}
$("[data-primary-download]").addEventListener("click", (event) =>
  download(event, system.os, recommendedChoice(system)),
);
for (const link of document.querySelectorAll("[data-installer]"))
  link.addEventListener("click", (event) =>
    download(
      event,
      link.dataset.installer,
      $(`[data-choice="${link.dataset.installer}"]`).value,
    ),
  );

async function enhance() {
  const jobs = [
    getLatest().then((value) => {
      release = value;
      live = true;
      renderDownloads();
    }),
    fetch(API_URL, {
      headers: { Accept: "application/vnd.github+json" },
      signal: AbortSignal.timeout(8000),
    })
      .then((response) => {
        if (!response.ok) throw new Error("Repository lookup unavailable.");
        return response.json();
      })
      .then((repository) => {
        if (
          !Number.isInteger(repository.stargazers_count) ||
          repository.stargazers_count < 0
        )
          return;
        const count = new Intl.NumberFormat("en", {
          notation: "compact",
          maximumFractionDigits: 1,
        }).format(repository.stargazers_count);
        $("[data-stars]").textContent = count;
        $(".github-button").setAttribute(
          "aria-label",
          `Star FolderSkin on GitHub, ${repository.stargazers_count} stars`,
        );
      }),
  ];
  if (navigator.userAgentData?.getHighEntropyValues)
    jobs.push(
      navigator.userAgentData
        .getHighEntropyValues(["architecture", "bitness"])
        .then((hints) => {
          system = detectSystem(navigator, hints);
          const choice = recommendedChoice(system);
          if (choice && system.os === "linux")
            $('[data-choice="linux"]').value = choice;
          renderDownloads();
        }),
    );
  await Promise.allSettled(jobs);
  if (!live)
    $("[data-version-link]").firstChild.textContent =
      `${release.tag} listed · See latest release `;
}
const initialChoice = recommendedChoice(system);
if (initialChoice && system.os === "linux")
  $('[data-choice="linux"]').value = initialChoice;
renderDownloads();
void enhance();
