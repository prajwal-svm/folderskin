/**
 * Brings the install counts of renamed packs along to their new ids, once folderskin-community's
 * rename has landed. It reads that repository's moved.json and prints the SQL for D1:
 *
 *   node scripts/moved-installs.mjs <path to folderskin-community>/moved.json > /tmp/moved-installs.sql
 *   pnpm exec wrangler d1 execute folderskin-community --remote --file=/tmp/moved-installs.sql
 *
 * Each old id's count is added to its new id's (summed when both have one) and the old row is
 * deleted; today's once-a-network records move too, so an add under the old id and one under the
 * new still count once today. Running it a second time changes nothing, so a run that stops
 * halfway can simply be run again. From the moment index.json lists a rename, the service counts
 * an add under the old id toward the new one by itself (src/installs.ts); this is for the counts
 * from before.
 *
 * Plain JavaScript with no dependencies, so any Node from 18 on runs it. The tests import
 * `movedInstallsSql` and run what it prints against D1 (test/moved.test.ts).
 */

/** A pack id (`pack::is_pack_id`, and isPackId in src/text.ts): the only thing put into the SQL. */
function isPackId(id) {
  return (
    typeof id === "string" &&
    id.length <= 40 &&
    /^[a-z0-9]+(-[a-z0-9]+)*$/.test(id) &&
    !/^(con|prn|aux|nul|com\d|lpt\d)$/.test(id)
  );
}

/**
 * The SQL that moves the counts, for moved.json's contents (`{"version": 1, "moved": {"<old id>":
 * "<new id>"}}`, parsed). Throws an Error saying what is wrong with a file that isn't that, or
 * that breaks its rules: every key and value a pack id, no id renamed to itself, and no chains, so
 * a new id is never itself renamed.
 */
export function movedInstallsSql(file) {
  const moved = file && typeof file === "object" ? file.moved : undefined;
  if (file?.version !== 1 || !moved || typeof moved !== "object" || Array.isArray(moved)) {
    throw new Error('moved.json should be {"version": 1, "moved": {"<old id>": "<new id>", …}}');
  }
  const renames = Object.entries(moved);
  if (renames.length === 0) throw new Error("moved.json renames nothing, so there are no counts to move");
  for (const [from, to] of renames) {
    if (!isPackId(from) || !isPackId(to)) throw new Error(`${JSON.stringify(from)} → ${JSON.stringify(to)} isn't an old and a new pack id`);
    if (from === to) throw new Error(`${from} is renamed to itself`);
    if (Object.hasOwn(moved, to)) throw new Error(`${from} → ${to}, but ${to} is renamed too: moved.json never chains`);
  }
  const lines = [
    `-- The install counts of ${renames.length} renamed packs, moved to their new ids from moved.json by`,
    "-- services/community/scripts/moved-installs.mjs. Running it again changes nothing.",
  ];
  for (const [from, to] of renames) {
    lines.push(
      "",
      `-- ${from} is ${to} now.`,
      // The WHERE keeps SQLite from reading the upsert's ON as a join's.
      `INSERT INTO installs (pack, n) SELECT '${to}', n FROM installs WHERE pack = '${from}' AND n > 0 ON CONFLICT (pack) DO UPDATE SET n = n + excluded.n;`,
      `DELETE FROM installs WHERE pack = '${from}';`,
      // A network that added the pack under both ids today is counted once, as it would have been.
      `UPDATE OR IGNORE installs_seen SET pack = '${to}' WHERE pack = '${from}';`,
      `DELETE FROM installs_seen WHERE pack = '${from}';`,
    );
  }
  return `${lines.join("\n")}\n`;
}

// Run as a script, rather than imported by the tests: read the file it is given and print the SQL.
if (/moved-installs\.mjs$/.test(globalThis.process?.argv?.[1] ?? "")) {
  const { readFileSync } = await import("node:fs");
  const path = process.argv[2];
  if (!path) {
    console.error("Usage: node scripts/moved-installs.mjs <path to folderskin-community>/moved.json > moved-installs.sql");
    process.exit(2);
  }
  try {
    process.stdout.write(movedInstallsSql(JSON.parse(readFileSync(path, "utf8"))));
  } catch (e) {
    console.error(`moved-installs: ${e instanceof Error ? e.message : e}`);
    process.exit(1);
  }
}
