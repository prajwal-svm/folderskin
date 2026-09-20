export const REPOSITORY = "prajwal-svm/folderskin";
export const RELEASES_URL = `https://github.com/${REPOSITORY}/releases/latest`;
export const API_URL = `https://api.github.com/repos/${REPOSITORY}`;

export function detectSystem(
  { userAgent = "", platform = "", maxTouchPoints = 0, userAgentData } = {},
  hints = {},
) {
  const platformName = userAgentData?.platform || platform;
  if (
    userAgentData?.mobile ||
    /Android|iPhone|iPad|iPod/i.test(userAgent) ||
    (/Mac/i.test(platformName) && maxTouchPoints > 1)
  ) {
    return { os: null, architecture: null, mobile: true };
  }
  const identity = `${platformName} ${userAgent}`;
  const os = /Windows|Win32|Win64/i.test(identity)
    ? "windows"
    : /Mac/i.test(identity)
      ? "macos"
      : /Linux/i.test(identity) && !/CrOS/i.test(identity)
        ? "linux"
        : null;
  let architecture = /arm|aarch64/i.test(hints.architecture || identity)
    ? "arm64"
    : "x64";
  if (
    os === "windows" &&
    /Win32/i.test(platformName) &&
    !/WOW64|Win64|x64|arm|aarch64/i.test(userAgent) &&
    hints.bitness === "32"
  )
    architecture = "x86";
  if (os === "macos") architecture = "universal";
  return { os, architecture, mobile: false };
}

export function parseInstaller(asset) {
  if (
    !asset ||
    typeof asset.name !== "string" ||
    typeof asset.browser_download_url !== "string"
  )
    return null;
  // Only published installers from this repository can become download targets.
  if (
    !asset.browser_download_url.startsWith(
      `https://github.com/${REPOSITORY}/releases/download/`,
    )
  )
    return null;
  const name = asset.name.toLowerCase();
  if (!/^folderskin[_-]/.test(name)) return null;
  const format = name.match(/\.(dmg|exe|msi|appimage|deb|rpm)$/)?.[1];
  if (!format) return null;
  const os =
    format === "dmg"
      ? "macos"
      : ["exe", "msi"].includes(format)
        ? "windows"
        : "linux";
  const architecture = /(?:_|-|\.)universal(?:_|-|\.)/.test(name)
    ? "universal"
    : /(?:_|-|\.)(arm64|aarch64)(?:_|-|\.)/.test(name)
      ? "arm64"
      : /(?:_|-|\.)(amd64|x86_64|x64)(?:_|-|\.)/.test(name)
        ? "x64"
        : /(?:_|-|\.)(i386|i686|x86)(?:_|-|\.)/.test(name)
          ? "x86"
          : null;
  if (!architecture) return null;
  return {
    name: asset.name,
    browser_download_url: asset.browser_download_url,
    size: asset.size,
    os,
    architecture,
    format,
  };
}

export function normalizeRelease(data) {
  if (
    !data ||
    data.draft ||
    data.prerelease ||
    typeof data.tag_name !== "string" ||
    !Array.isArray(data.assets)
  )
    throw new Error("No stable release is available.");
  const assets = data.assets.map(parseInstaller).filter(Boolean);
  if (!assets.length)
    throw new Error("This release has no supported installers.");
  return { tag: data.tag_name, assets };
}

export function selectInstaller(release, os, architecture, format) {
  return (
    release?.assets.find(
      (asset) =>
        asset.os === os &&
        asset.architecture === architecture &&
        asset.format === format,
    ) || null
  );
}

export function recommendedChoice(system) {
  if (system.os === "macos") return "universal:dmg";
  // Windows ARM can run the published x64 installer through Windows emulation.
  if (system.os === "windows" && system.architecture !== "x86")
    return "x64:exe";
  if (system.os === "linux") return `${system.architecture}:appimage`;
  return null;
}

export async function fetchLatestRelease(fetcher = fetch) {
  const response = await fetcher(`${API_URL}/releases/latest`, {
    headers: { Accept: "application/vnd.github+json" },
    cache: "no-cache",
    signal: AbortSignal.timeout(8000),
  });
  if (!response.ok)
    throw new Error(`Release lookup failed (${response.status}).`);
  return normalizeRelease(await response.json());
}

export function formatSize(bytes) {
  return Number.isFinite(bytes) ? `${(bytes / 1_000_000).toFixed(1)} MB` : "";
}
