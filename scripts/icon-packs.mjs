#!/usr/bin/env node
/**
 * Builds the composer's icon packs from pinned releases of open-source icon sets.
 *
 *   node scripts/icon-packs.mjs lucide          the built-in pack, written into src/composer/icons/
 *   node scripts/icon-packs.mjs --all           every pack into dist-icons/, and the catalog the app reads
 *   node scripts/icon-packs.mjs --all --verify  the same, failing if any pack differs from the catalog
 *
 * Each set is fetched from npm at a pinned version and checked against its published integrity
 * hash, so a pack is built from exactly what was reviewed. Every SVG element becomes plain path
 * data, which the canvas draws with Path2D: no SVG images, which the app's content security
 * policy wouldn't load and a canvas can't always export. The output is deterministic, so the
 * release workflow can rebuild a pack and check it against the SHA-256 the app already carries.
 * Nothing here depends on anything outside Node itself.
 */
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const CACHE = join(ROOT, "node_modules", ".cache", "folderskin-icons");
/** The release the downloadable packs are published under; the app builds its download URLs from it. */
const RELEASE = "icons-v1";

/**
 * The packs. `dir` is where the pack's SVGs sit in the npm tarball; `style` is how they're
 * drawn (stroked lines or filled shapes); `brands` marks logos that belong to someone else.
 */
const PACKS = [
  {
    id: "lucide", name: "Lucide", pkg: "lucide-static", version: "1.47.0",
    integrity: "sha512-yWIrkdXc688Feq5VjOktsKmV5Ikc7y5Nu3rrdtbr8nWjkJWk8QlnZfVtIak22Af+fNhZ7k4cTJpZo1zmj7X5sA==",
    license: "ISC", source: "https://lucide.dev", dir: "package/icons/", style: "stroke", tags: "lucide", builtin: true,
  },
  {
    id: "tabler", name: "Tabler", pkg: "@tabler/icons", version: "3.48.0",
    integrity: "sha512-lQk06gVHNVBnJp0UOgtth54mcZBJM7rdYKsAxKOProj1bbSAx+/xiuOWfHlyvgdL3M+Y+I0HINrjHa3ygp8eXQ==",
    license: "MIT", source: "https://tabler.io/icons", dir: "package/icons/outline/", style: "stroke", tags: "tabler",
  },
  {
    id: "phosphor", name: "Phosphor", pkg: "@phosphor-icons/core", version: "2.1.1",
    integrity: "sha512-v4ARvrip4qBCImOE5rmPUylOEK4iiED9ZyKjcvzuezqMaiRASCHKcRIuvvxL/twvLpkfnEODCOJp5dM4eZilxQ==",
    license: "MIT", source: "https://phosphoricons.com", dir: "package/assets/regular/", style: "fill", tags: "phosphor",
  },
  {
    id: "phosphor-fill", name: "Phosphor Fill", pkg: "@phosphor-icons/core", version: "2.1.1",
    integrity: "sha512-v4ARvrip4qBCImOE5rmPUylOEK4iiED9ZyKjcvzuezqMaiRASCHKcRIuvvxL/twvLpkfnEODCOJp5dM4eZilxQ==",
    license: "MIT", source: "https://phosphoricons.com", dir: "package/assets/fill/", style: "fill", tags: "phosphor", suffix: "-fill",
  },
  {
    id: "heroicons", name: "Heroicons", pkg: "heroicons", version: "2.2.0",
    integrity: "sha512-yOwvztmNiBWqR946t+JdgZmyzEmnRMC2nxvHFC90bF1SUttwB6yJKYeme1JeEcBfobdOs827nCyiWBS2z/brog==",
    license: "MIT", source: "https://heroicons.com", dir: "package/24/outline/", style: "stroke",
  },
  {
    id: "heroicons-solid", name: "Heroicons Solid", pkg: "heroicons", version: "2.2.0",
    integrity: "sha512-yOwvztmNiBWqR946t+JdgZmyzEmnRMC2nxvHFC90bF1SUttwB6yJKYeme1JeEcBfobdOs827nCyiWBS2z/brog==",
    license: "MIT", source: "https://heroicons.com", dir: "package/24/solid/", style: "fill",
  },
  {
    id: "iconoir", name: "Iconoir", pkg: "iconoir", version: "7.12.1",
    integrity: "sha512-7ei4jd1bss0Ukyz/bbM9Zc96aLfZuwTEkSW8u5fx5h3X9912MsnmWwPw9LyTUtcf0ShfTzQeQ9oG7IslW3hrLg==",
    license: "MIT", source: "https://iconoir.com", dir: "package/icons/regular/", style: "stroke",
  },
  {
    id: "bootstrap", name: "Bootstrap Icons", pkg: "bootstrap-icons", version: "1.13.1",
    integrity: "sha512-ijombt4v6bv5CLeXvRWKy7CuM3TRTuPEuGaGKvTV5cz65rQSY8RQ2JcHt6b90cBBAC7s8fsf2EkQDldzCoXUjw==",
    license: "MIT", source: "https://icons.getbootstrap.com", dir: "package/icons/", style: "fill",
  },
  {
    id: "lobe", name: "Lobe Icons", pkg: "@lobehub/icons-static-svg", version: "1.95.1",
    integrity: "sha512-Hw7EPPgVnC4NZLXBfTNJG6hyQgqECfUPC11VVXodPSr1aebKcFxDZlSpxhWwYNdCc6bhxps/x5TtXoPmfKH2ag==",
    license: "MIT", source: "https://lobehub.com/icons", dir: "package/icons/", style: "fill", brands: true,
    // The single-colour logos; the coloured, lettered and combined variants are separate files.
    skip: /-(color|text|brand|combine)(-[a-z]+)?$/,
  },
  {
    id: "simple-icons", name: "Simple Icons", pkg: "simple-icons", version: "16.32.0",
    integrity: "sha512-BwqATHxAulx7X6kNdTkecy7PBjLkgtAHcgrwYLd9iA+cD2At9DzhJwdwaSELk2+aZe01IfiM/Wxtyp68Dvup2A==",
    license: "CC0-1.0", source: "https://simpleicons.org", dir: "package/icons/", style: "fill", tags: "simple", brands: true,
  },
];

// ---------- fetching ----------

async function tarball(pack) {
  mkdirSync(CACHE, { recursive: true });
  const file = join(CACHE, `${pack.pkg.replace("/", "__")}-${pack.version}.tgz`);
  let bytes = existsSync(file) ? readFileSync(file) : null;
  if (!bytes) {
    const base = pack.pkg.startsWith("@") ? pack.pkg.split("/")[1] : pack.pkg;
    const url = `https://registry.npmjs.org/${pack.pkg}/-/${base}-${pack.version}.tgz`;
    const res = await fetch(url);
    if (!res.ok) throw new Error(`${url} answered ${res.status}`);
    bytes = Buffer.from(await res.arrayBuffer());
  }
  const [algo, want] = pack.integrity.split("-");
  const got = createHash(algo).update(bytes).digest("base64");
  if (got !== want) throw new Error(`${pack.pkg}@${pack.version} doesn't match its pinned integrity hash`);
  writeFileSync(file, bytes);
  return untar(gunzipSync(bytes));
}

/** The files in a tar archive, by name. Only regular files; enough for npm tarballs. */
function untar(buf) {
  const files = new Map();
  for (let off = 0; off + 512 <= buf.length; ) {
    const header = buf.subarray(off, off + 512);
    if (header.every((b) => b === 0)) break;
    const field = (a, b) => header.subarray(a, b).toString("utf8").replace(/\0.*$/s, "");
    const name = (field(345, 500) ? `${field(345, 500)}/` : "") + field(0, 100);
    const size = parseInt(field(124, 136).trim() || "0", 8);
    const type = String.fromCharCode(header[156] || 48);
    if (type === "0") files.set(name.replace(/^\.\//, ""), buf.subarray(off + 512, off + 512 + size));
    off += 512 + Math.ceil(size / 512) * 512;
  }
  return files;
}

// ---------- tags ----------

function readTags(pack, files) {
  const tags = new Map();
  const add = (name, list) => {
    const words = [...new Set(list.map((t) => String(t).toLowerCase().trim()).filter((t) => t && t !== "*new*" && t !== name))];
    if (words.length) tags.set(name, words.slice(0, 16));
  };
  const text = (path) => files.get(path)?.toString("utf8");
  if (pack.tags === "lucide") {
    for (const [name, list] of Object.entries(JSON.parse(text("package/tags.json")))) add(name, list);
  } else if (pack.tags === "tabler") {
    for (const [name, v] of Object.entries(JSON.parse(text("package/icons.json")))) add(name, [...(v.tags ?? []), v.category ?? ""]);
  } else if (pack.tags === "phosphor") {
    const js = text("package/dist/index.mjs");
    for (const m of js.matchAll(/name: "([a-z0-9-]+)",[\s\S]*?tags: \[([\s\S]*?)\]/g)) {
      add(m[1] + (pack.suffix ?? ""), [...m[2].matchAll(/"([^"]+)"/g)].map((t) => t[1]));
    }
  } else if (pack.tags === "simple") {
    const data = JSON.parse(text("package/data/simple-icons.json"));
    for (const icon of Array.isArray(data) ? data : data.icons) {
      const slug = icon.slug ?? slugOf(icon.title);
      const aliases = icon.aliases ? [...(icon.aliases.aka ?? []), ...Object.values(icon.aliases.loc ?? {})] : [];
      add(slug, [icon.title, ...aliases]);
      if (icon.hex) tags.set(`${slug}#`, `#${icon.hex.toLowerCase()}`);
    }
  }
  return tags;
}

/** Simple Icons' own rule for turning a title into a file name. */
function slugOf(title) {
  const map = { "+": "plus", ".": "dot", "&": "and", đ: "d", ħ: "h", ı: "i", ĸ: "k", ŀ: "l", ł: "l", ß: "ss", ŧ: "t" };
  return title
    .toLowerCase()
    .replace(/[+.&đħıĸŀłßŧ]/g, (c) => map[c])
    .normalize("NFD")
    .replace(/[^a-z0-9]/g, "");
}

// ---------- SVG to path data ----------

const SHAPES = /<(path|circle|ellipse|rect|line|polyline|polygon)\b([^>]*?)\/?>/g;
/** What a pack's drawing may not use: things a path can't say. Icons with them are left out. */
const UNSUPPORTED = /<(linearGradient|radialGradient|filter|mask|clipPath|image|text|use|pattern)\b|transform=/;

const attrsOf = (s) => Object.fromEntries([...s.matchAll(/([\w:-]+)="([^"]*)"/g)].map((m) => [m[1], m[2]]));
const n = (v, d = 0) => (v === undefined || v === "" ? d : Number(v));
const fmt = (x) => String(Math.round(x * 1000) / 1000);

function pathOf(tag, a) {
  switch (tag) {
    case "path":
      return a.d?.trim() || null;
    case "circle":
    case "ellipse": {
      const cx = n(a.cx), cy = n(a.cy);
      const rx = tag === "circle" ? n(a.r) : n(a.rx), ry = tag === "circle" ? n(a.r) : n(a.ry);
      if (!(rx > 0 && ry > 0)) return null;
      return `M${fmt(cx - rx)} ${fmt(cy)}a${fmt(rx)} ${fmt(ry)} 0 1 0 ${fmt(2 * rx)} 0a${fmt(rx)} ${fmt(ry)} 0 1 0 ${fmt(-2 * rx)} 0Z`;
    }
    case "rect": {
      const x = n(a.x), y = n(a.y), w = n(a.width), h = n(a.height);
      if (!(w > 0 && h > 0)) return null;
      let rx = n(a.rx, NaN), ry = n(a.ry, NaN);
      if (Number.isNaN(rx)) rx = Number.isNaN(ry) ? 0 : ry;
      if (Number.isNaN(ry)) ry = rx;
      rx = Math.min(rx, w / 2);
      ry = Math.min(ry, h / 2);
      if (rx <= 0 || ry <= 0) return `M${fmt(x)} ${fmt(y)}h${fmt(w)}v${fmt(h)}h${fmt(-w)}Z`;
      return (
        `M${fmt(x + rx)} ${fmt(y)}h${fmt(w - 2 * rx)}a${fmt(rx)} ${fmt(ry)} 0 0 1 ${fmt(rx)} ${fmt(ry)}` +
        `v${fmt(h - 2 * ry)}a${fmt(rx)} ${fmt(ry)} 0 0 1 ${fmt(-rx)} ${fmt(ry)}h${fmt(-(w - 2 * rx))}` +
        `a${fmt(rx)} ${fmt(ry)} 0 0 1 ${fmt(-rx)} ${fmt(-ry)}v${fmt(-(h - 2 * ry))}a${fmt(rx)} ${fmt(ry)} 0 0 1 ${fmt(rx)} ${fmt(-ry)}Z`
      );
    }
    case "line":
      return `M${fmt(n(a.x1))} ${fmt(n(a.y1))}L${fmt(n(a.x2))} ${fmt(n(a.y2))}`;
    case "polyline":
    case "polygon": {
      const p = (a.points ?? "").trim().split(/[\s,]+/).map(Number);
      if (p.length < 4 || p.some(Number.isNaN)) return null;
      let d = `M${fmt(p[0])} ${fmt(p[1])}`;
      for (let i = 2; i + 1 < p.length; i += 2) d += `L${fmt(p[i])} ${fmt(p[i + 1])}`;
      return tag === "polygon" ? `${d}Z` : d;
    }
  }
  return null;
}

/** One icon from its SVG: its viewBox side, its paths, which of them are filled, and its fill rule. */
function iconOf(svg, style) {
  if (UNSUPPORTED.test(svg)) return null;
  const root = attrsOf(svg.match(/<svg\b([^>]*)>/)?.[1] ?? "");
  const vb = (root.viewBox ?? "0 0 24 24").trim().split(/[\s,]+/).map(Number);
  if (vb.length !== 4 || vb[0] !== 0 || vb[1] !== 0 || vb[2] !== vb[3]) return null;
  const d = [];
  const filled = [];
  let evenOdd = false;
  for (const m of svg.matchAll(SHAPES)) {
    const a = attrsOf(m[2]);
    const stroke = a.stroke ?? root.stroke;
    const fill = a.fill ?? root.fill;
    if (style === "stroke") {
      // A path with no stroke in a line icon is a bounding box (Tabler) unless it's filled on purpose.
      const isFilled = fill && fill !== "none";
      if (stroke === "none" && !isFilled) continue;
      const path = pathOf(m[1], a);
      if (!path) continue;
      if (isFilled && stroke === "none") filled.push(d.length);
      d.push(path);
    } else {
      if (fill === "none") continue;
      const path = pathOf(m[1], a);
      if (!path) continue;
      d.push(path);
    }
    if ((a["fill-rule"] ?? root["fill-rule"]) === "evenodd") evenOdd = true;
  }
  if (d.length === 0) return null;
  const strokeWidth = n(root["stroke-width"], style === "stroke" ? 2 : 0);
  return { viewBox: vb[2], d, filled, evenOdd, strokeWidth };
}

async function build(pack) {
  const files = await tarball(pack);
  const tags = readTags(pack, files);
  const icons = [];
  let viewBox = 0;
  let strokeWidth = 0;
  let left = 0;
  for (const [path, bytes] of files) {
    if (!path.startsWith(pack.dir) || !path.endsWith(".svg") || path.slice(pack.dir.length).includes("/")) continue;
    const name = path.slice(pack.dir.length, -4);
    if (pack.skip?.test(name)) continue;
    const icon = iconOf(bytes.toString("utf8"), pack.style);
    if (!icon || (viewBox && icon.viewBox !== viewBox)) {
      left += 1;
      continue;
    }
    viewBox ||= icon.viewBox;
    strokeWidth ||= icon.strokeWidth;
    const entry = { n: name, d: icon.d };
    const t = tags.get(name);
    if (t) entry.t = t;
    if (icon.filled.length) entry.f = icon.filled;
    if (icon.evenOdd) entry.r = 1;
    const colour = tags.get(`${name}#`);
    if (colour) entry.c = colour;
    icons.push(entry);
  }
  icons.sort((a, b) => (a.n < b.n ? -1 : a.n > b.n ? 1 : 0));
  const out = {
    format: 1,
    id: pack.id,
    name: pack.name,
    version: pack.version,
    license: pack.license,
    source: pack.source,
    style: pack.style,
    viewBox,
    strokeWidth,
    brands: pack.brands === true,
    icons,
  };
  return { json: JSON.stringify(out), count: icons.length, left };
}

const sha256 = (s) => createHash("sha256").update(s).digest("hex");

// ---------- main ----------

const args = process.argv.slice(2);
const all = args.includes("--all");
const verify = args.includes("--verify");
const wanted = all ? PACKS : PACKS.filter((p) => args.includes(p.id));
if (wanted.length === 0) {
  console.error("Name a pack (lucide, tabler, …) or pass --all.");
  process.exit(1);
}

const catalogPath = join(ROOT, "src", "composer", "icons", "catalog.ts");
const catalog = [];
let failed = false;
for (const pack of wanted) {
  const { json, count, left } = await build(pack);
  const bytes = Buffer.byteLength(json);
  const hash = sha256(json);
  if (pack.builtin) {
    writeFileSync(join(ROOT, "src", "composer", "icons", `${pack.id}.json`), json);
  } else {
    mkdirSync(join(ROOT, "dist-icons"), { recursive: true });
    writeFileSync(join(ROOT, "dist-icons", `${pack.id}.json`), json);
  }
  catalog.push({ id: pack.id, name: pack.name, version: pack.version, license: pack.license, source: pack.source, style: pack.style, brands: pack.brands === true, builtin: pack.builtin === true, count, bytes, sha256: hash });
  console.log(`${pack.id}: ${count} icons, ${(bytes / 1024).toFixed(0)} KB${left ? `, ${left} left out (gradients, masks or transforms)` : ""}`);
  if (verify && existsSync(catalogPath) && !readFileSync(catalogPath, "utf8").includes(hash)) {
    console.error(`${pack.id} built to ${hash}, which isn't the hash in src/composer/icons/catalog.ts`);
    failed = true;
  }
}

if (all && !verify) {
  const ts = `// Written by scripts/icon-packs.mjs --all; don't edit by hand. Rebuild after changing a pack there.

/** One icon pack the composer offers: built in, or downloaded from the \`${RELEASE}\` release. */
export type IconPackInfo = {
  id: string;
  name: string;
  version: string;
  license: string;
  source: string;
  style: "stroke" | "fill";
  /** Logos that belong to their owners: fine on your own folders, not in a shared pack. */
  brands: boolean;
  builtin: boolean;
  count: number;
  bytes: number;
  sha256: string;
};

/** The release the downloadable packs are published under. */
export const ICON_RELEASE = ${JSON.stringify(RELEASE)};

export const ICON_PACKS: IconPackInfo[] = ${JSON.stringify(catalog, null, 2)};
`;
  writeFileSync(catalogPath, ts);
  console.log(`wrote ${catalogPath}`);
}
if (failed) process.exit(1);
