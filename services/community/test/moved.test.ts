import { env } from "cloudflare:workers";
import { describe, expect, it } from "vitest";
import { movedInstallsSql } from "../scripts/moved-installs.mjs";
import { dayOf, now } from "../src/bytes";

/** Runs the script's SQL the way `wrangler d1 execute --file` does: every statement, in order. */
async function run(sql: string) {
  const statements = sql.split("\n").filter((line) => line.trim() !== "" && !line.startsWith("--"));
  await env.DB.batch(statements.map((statement) => env.DB.prepare(statement)));
}

async function installs(...packs: string[]) {
  const { results } = await env.DB.prepare(`SELECT pack, n FROM installs WHERE pack IN (${packs.map((_, i) => `?${i + 1}`).join(", ")}) ORDER BY pack`)
    .bind(...packs)
    .all<{ pack: string; n: number }>();
  return Object.fromEntries(results.map((r) => [r.pack, r.n]));
}

describe("moving install counts to renamed packs' new ids", () => {
  it("adds each old id's count to its new id's, and moves today's adds with it", async () => {
    const today = dayOf(now());
    await env.DB.batch([
      env.DB.prepare("INSERT INTO installs (pack, n) VALUES ('classic-art', 5), ('classic-art-k7q2mx', 2), ('koi-ponds', 3), ('untouched', 9)"),
      env.DB.prepare("INSERT INTO installs_seen (day, network, pack) VALUES (?1, 'n1', 'classic-art'), (?1, 'n1', 'classic-art-k7q2mx'), (?1, 'n2', 'koi-ponds')").bind(today),
    ]);
    const sql = movedInstallsSql({ version: 1, moved: { "classic-art": "classic-art-k7q2mx", "koi-ponds": "koi-ponds-ab2cd3", "never-added": "never-added-zz2345" } });
    await run(sql);
    const counts = { "classic-art-k7q2mx": 7, "koi-ponds-ab2cd3": 3, untouched: 9 };
    expect(await installs("classic-art", "classic-art-k7q2mx", "koi-ponds", "koi-ponds-ab2cd3", "never-added", "never-added-zz2345", "untouched")).toEqual(counts);
    const { results: seen } = await env.DB.prepare("SELECT network, pack FROM installs_seen WHERE day = ?1 ORDER BY network, pack").bind(today).all();
    expect(seen).toEqual([
      { network: "n1", pack: "classic-art-k7q2mx" },
      { network: "n2", pack: "koi-ponds-ab2cd3" },
    ]);

    // A second run changes nothing.
    await run(sql);
    expect(await installs("classic-art", "classic-art-k7q2mx", "koi-ponds", "koi-ponds-ab2cd3", "untouched")).toEqual(counts);
  });

  it("prints a statement at a time, with the rename it is for", () => {
    const sql = movedInstallsSql({ version: 1, moved: { "old-pack": "old-pack-abcdef" } });
    expect(sql).toContain("-- old-pack is old-pack-abcdef now.");
    expect(sql.split("\n").filter((line) => line.endsWith(";"))).toHaveLength(4);
  });

  it("won't take a file that isn't moved.json, or that breaks its rules", () => {
    const bad: [unknown, RegExp][] = [
      [null, /should be/],
      [{ version: 2, moved: { a: "b" } }, /should be/],
      [{ version: 1, moved: [] }, /should be/],
      [{ version: 1, moved: {} }, /renames nothing/],
      [{ version: 1, moved: { "x'; DROP TABLE installs; --": "koi-abcdef" } }, /isn't an old and a new pack id/],
      [{ version: 1, moved: { koi: 5 } }, /isn't an old and a new pack id/],
      [{ version: 1, moved: { koi: "koi" } }, /renamed to itself/],
      [{ version: 1, moved: { a: "b", b: "c" } }, /never chains/],
    ];
    for (const [file, why] of bad) expect(() => movedInstallsSql(file), JSON.stringify(file)).toThrow(why);
  });
});
