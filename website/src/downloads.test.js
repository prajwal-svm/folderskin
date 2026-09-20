import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  detectSystem,
  recommendedChoice,
  parseInstaller,
  normalizeRelease,
  selectInstaller,
  fetchLatestRelease,
} from "./downloads.js";
const snapshot = JSON.parse(
  readFileSync(new URL("./release-snapshot.json", import.meta.url), "utf8"),
);
const fixture = snapshot.release;
const asset = (name) => ({
  name,
  size: 1000,
  browser_download_url: `https://github.com/prajwal-svm/folderskin/releases/download/v9.0.0/${name}`,
});

test("recommends the universal macOS installer for Apple silicon and Intel browsers", () => {
  for (const userAgent of [
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
    "Mozilla/5.0 (Macintosh; ARM Mac OS X)",
  ]) {
    const system = detectSystem({ userAgent, platform: "MacIntel" });
    assert.equal(system.os, "macos");
    assert.equal(recommendedChoice(system), "universal:dmg");
  }
});
test("does not mistake phones, tablets, or ChromeOS for desktop downloads", () => {
  for (const environment of [
    { userAgent: "Mozilla iPhone", platform: "iPhone" },
    { userAgent: "Mozilla Android Linux aarch64", platform: "Linux armv8l" },
    {
      userAgent: "Mozilla Macintosh Intel Mac OS X",
      platform: "MacIntel",
      maxTouchPoints: 5,
    },
    { userAgent: "Mozilla X11 CrOS x86_64", platform: "Linux x86_64" },
    { userAgentData: { mobile: true, platform: "Android" } },
    {},
  ])
    assert.equal(detectSystem(environment).os, null);
});
test("uses architecture hints for Linux and explains Windows ARM through its system state", () => {
  const linux = detectSystem(
    { userAgent: "Mozilla Linux x86_64", userAgentData: { platform: "Linux" } },
    { architecture: "arm" },
  );
  assert.equal(recommendedChoice(linux), "arm64:appimage");
  const windows = detectSystem(
    { userAgentData: { platform: "Windows" } },
    { architecture: "arm" },
  );
  assert.equal(windows.architecture, "arm64");
  assert.equal(recommendedChoice(windows), "x64:exe");
  assert.equal(
    recommendedChoice(
      detectSystem(
        { platform: "Win32", userAgent: "Windows NT 10.0" },
        { bitness: "32" },
      ),
    ),
    null,
  );
});
test("matches all nine published installer formats and architectures exactly", () => {
  for (const [os, architecture, format, expected] of [
    ["macos", "universal", "dmg", /universal\.dmg$/],
    ["windows", "x64", "exe", /x64-setup\.exe$/],
    ["windows", "x64", "msi", /x64_en-US\.msi$/],
    ["linux", "x64", "appimage", /amd64\.AppImage$/],
    ["linux", "arm64", "appimage", /aarch64\.AppImage$/],
    ["linux", "x64", "deb", /amd64\.deb$/],
    ["linux", "arm64", "deb", /arm64\.deb$/],
    ["linux", "x64", "rpm", /x86_64\.rpm$/],
    ["linux", "arm64", "rpm", /aarch64\.rpm$/],
  ])
    assert.match(
      selectInstaller(fixture, os, architecture, format).name,
      expected,
    );
  assert.equal(selectInstaller(fixture, "windows", "arm64", "exe"), null);
});
test("never selects signatures, update archives, unknown architectures, or foreign URLs", () => {
  for (const name of [
    "FolderSkin_9.0.0_universal.dmg.sig",
    "FolderSkin_9.0.0_universal.app.tar.gz",
    "latest.json",
    "FolderSkin_9.0.0.deb",
    "other_9.0.0_x64.exe",
  ])
    assert.equal(parseInstaller(asset(name)), null);
  assert.equal(
    parseInstaller({
      ...asset("FolderSkin_9.0.0_x64.exe"),
      browser_download_url: "https://example.com/file.exe",
    }),
    null,
  );
});
test("rejects drafts, prereleases, and releases without installers", () => {
  const release = {
    tag_name: "v9.0.0",
    assets: [asset("FolderSkin_9.0.0_universal.dmg")],
  };
  assert.equal(normalizeRelease(release).tag, "v9.0.0");
  for (const data of [
    { ...release, draft: true },
    { ...release, prerelease: true },
    { ...release, assets: [] },
    null,
  ])
    assert.throws(() => normalizeRelease(data));
});
test("resolves a newer release at request time instead of using the build snapshot", async () => {
  let requests = 0;
  const fetcher = async (url, options) => {
    assert.match(url, /\/releases\/latest$/);
    assert.equal(options.cache, "no-cache");
    requests++;
    return {
      ok: true,
      json: async () => ({
        tag_name: `v9.0.${requests}`,
        assets: [asset(`FolderSkin_9.0.${requests}_universal.dmg`)],
      }),
    };
  };
  assert.equal((await fetchLatestRelease(fetcher)).tag, "v9.0.1");
  assert.equal((await fetchLatestRelease(fetcher)).tag, "v9.0.2");
});
test("surfaces rate limits, empty releases, and network failures for the GitHub fallback", async () => {
  await assert.rejects(
    fetchLatestRelease(async () => ({ ok: false, status: 403 })),
    /403/,
  );
  await assert.rejects(
    fetchLatestRelease(async () => ({
      ok: true,
      json: async () => ({ assets: [] }),
    })),
  );
  await assert.rejects(
    fetchLatestRelease(async () => {
      throw new Error("offline");
    }),
    /offline/,
  );
});
