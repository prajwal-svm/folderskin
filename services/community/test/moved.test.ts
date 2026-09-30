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

  it("moves a pack's views and its daily counts too, and leaves a skin's alone", async () => {
    const today = dayOf(now());
    const yesterday = dayOf(now() - 86400);
    const skin = "ab".repeat(32);
    await env.DB.batch([
      env.DB.prepare(
        "INSERT INTO counts_total (kind, target, n) VALUES ('pack_view', 'old-views', 4), ('pack_view', 'old-views-ab2cd3', 1), ('skin_view', ?1, 6)",
      ).bind(skin),
      env.DB.prepare(
        `INSERT INTO counts_daily (kind, target, day, n) VALUES ('install', 'old-views', ?1, 2), ('pack_view', 'old-views', ?1, 3),
         ('pack_view', 'old-views-ab2cd3', ?1, 1), ('pack_view', 'old-views', ?2, 1), ('skin_download', ?3, ?1, 5)`,
      ).bind(today, yesterday, skin),
      env.DB.prepare(
        `INSERT INTO counts_seen (day, kind, network, target) VALUES (?1, 'pack_view', 'n1', 'old-views'),
         (?1, 'pack_view', 'n1', 'old-views-ab2cd3'), (?1, 'pack_view', 'n2', 'old-views')`,
      ).bind(today),
    ]);
    const sql = movedInstallsSql({ version: 1, moved: { "old-views": "old-views-ab2cd3" } });
    const rows = (table: string, columns: string, order: string) =>
      env.DB.prepare(`SELECT ${columns} FROM ${table} WHERE target IN ('old-views', 'old-views-ab2cd3', ?1) ORDER BY ${order}`).bind(skin).all();
    // A second run changes nothing.
    for (let i = 0; i < 2; i++) {
      await run(sql);
      const { results: totals } = await rows("counts_total", "kind, target, n", "kind, target");
      expect(totals).toEqual([
        { kind: "pack_view", target: "old-views-ab2cd3", n: 5 },
        { kind: "skin_view", target: skin, n: 6 },
      ]);
      const { results: days } = await rows("counts_daily", "kind, target, day, n", "day, kind");
      expect(days).toEqual([
        { kind: "pack_view", target: "old-views-ab2cd3", day: yesterday, n: 1 },
        { kind: "install", target: "old-views-ab2cd3", day: today, n: 2 },
        { kind: "pack_view", target: "old-views-ab2cd3", day: today, n: 4 },
        { kind: "skin_download", target: skin, day: today, n: 5 },
      ]);
      const { results: seen } = await rows("counts_seen", "network, target", "network");
      expect(seen).toEqual([
        { network: "n1", target: "old-views-ab2cd3" },
        { network: "n2", target: "old-views-ab2cd3" },
      ]);
    }
  });

  it("prints a statement at a time, with the rename it is for", () => {
    const sql = movedInstallsSql({ version: 1, moved: { "old-pack": "old-pack-abcdef" } });
    expect(sql).toContain("-- old-pack is old-pack-abcdef now.");
    expect(sql.split("\n").filter((line) => line.endsWith(";"))).toHaveLength(10);
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
