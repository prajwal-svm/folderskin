import { writeFile } from "node:fs/promises";
import { API_URL, normalizeRelease } from "../src/downloads.js";
const headers = { Accept: "application/vnd.github+json" };
if (process.env.GH_TOKEN)
  headers.Authorization = `Bearer ${process.env.GH_TOKEN}`;
async function get(path) {
  const response = await fetch(`${API_URL}${path}`, {
    headers,
    signal: AbortSignal.timeout(15000),
  });
  if (!response.ok) throw new Error(`GitHub returned ${response.status}`);
  return response.json();
}
const [release, repository] = await Promise.all([
  get("/releases/latest"),
  get(""),
]);
await writeFile(
  new URL("../src/release-snapshot.json", import.meta.url),
  `${JSON.stringify({ generatedAt: new Date().toISOString(), stars: repository.stargazers_count, release: normalizeRelease(release) }, null, 2)}\n`,
);
console.log(
  `Prepared ${release.tag_name} and ${repository.stargazers_count} GitHub stars.`,
);
