/**
 * The drive shapes FolderSkin draws (docs/DRIVES.md), as the webview names and places them. The
 * drives themselves are drawn in Rust (`crates/folderskin-core/src/drive`); this is the list of
 * them and where each one's face is, so a design can be laid out on a drive before its layers
 * have arrived, and a saved document can be checked without asking Rust.
 */
import type { Parts } from "./parts";

/** Whose drives a shape is drawn after. */
export type DriveStyle = "mac" | "windows" | "linux";

export const DRIVE_STYLES: DriveStyle[] = ["mac", "windows", "linux"];

/** Each system's drive kinds, in the order the composer lists them: the same lists as `DriveStyle::kinds`. */
export const DRIVE_KINDS: Record<DriveStyle, string[]> = {
  mac: ["startup", "internal", "external", "removable", "card", "optical", "disk-image", "network", "time-machine"],
  windows: ["startup", "internal", "removable", "card", "optical", "network"],
  linux: ["internal", "solid-state", "external", "removable", "card", "optical-drive", "optical", "server", "network", "multi-disk"],
};

/** Every drive's id, `mac-external`, system by system. */
export const DRIVE_IDS: string[] = DRIVE_STYLES.flatMap((style) => DRIVE_KINDS[style].map((kind) => `${style}-${kind}`));

export const isDriveId = (v: unknown): v is string => typeof v === "string" && DRIVE_IDS.includes(v);

/** The system a drive is drawn after. */
export const driveStyleOf = (id: string): DriveStyle => (id.startsWith("windows-") ? "windows" : id.startsWith("linux-") ? "linux" : "mac");

/** The kind of drive an id names: `external` for `mac-external`. */
export const driveKindOf = (id: string): string => id.slice(driveStyleOf(id).length + 1);

/**
 * The drive a new drive design starts on for a system: its external drive, the one most people
 * plug in, or on Windows its local disk, which is how Explorer draws an external one too
 * (`DriveShape::default_for`).
 */
export function defaultDrive(os: string): string {
  if (os === "windows") return "windows-internal";
  if (os === "linux") return "linux-external";
  return "mac-external";
}

type Box = [number, number, number, number];

/**
 * Where each drive's face is, in canvas units: its box, a point on it nothing hides, and the
 * drive's whole extent. The numbers `folderskin-tools composer-layers` writes to
 * docs/images/composer/drives/parts.json from the Rust drawing, which a test holds these to.
 */
export const DRIVE_FACES: Record<string, { face: Box; point: [number, number]; extent: Box }> = {
  "mac-startup": { face: [168, 92, 856, 820], point: [512, 456], extent: [166, 90, 858, 934] },
  "mac-internal": { face: [168, 92, 856, 820], point: [512, 456], extent: [166, 90, 858, 934] },
  "mac-external": { face: [168, 92, 856, 820], point: [512, 456], extent: [166, 90, 858, 934] },
  "mac-removable": { face: [168, 92, 856, 820], point: [512, 456], extent: [166, 90, 858, 934] },
  "mac-card": { face: [260, 276, 764, 866], point: [512, 571], extent: [204, 100, 804, 904] },
  "mac-optical": { face: [98, 98, 926, 926], point: [512, 223], extent: [84, 84, 940, 940] },
  "mac-disk-image": { face: [168, 92, 856, 820], point: [512, 456], extent: [166, 90, 858, 934] },
  "mac-network": { face: [168, 92, 856, 820], point: [512, 456], extent: [166, 90, 858, 934] },
  "mac-time-machine": { face: [168, 92, 856, 820], point: [512, 456], extent: [166, 90, 858, 934] },
  "windows-startup": { face: [93.25, 286, 930.75, 640], point: [512, 463], extent: [86, 284, 938, 748] },
  "windows-internal": { face: [93.25, 286, 930.75, 640], point: [512, 463], extent: [86, 284, 938, 748] },
  "windows-removable": { face: [75.25, 286, 836.75, 640], point: [456, 463], extent: [68, 284, 958, 748] },
  "windows-card": { face: [280, 312, 744, 856], point: [512, 584], extent: [236, 112, 788, 900] },
  "windows-optical": { face: [168, 98, 856, 786], point: [512, 203], extent: [86, 86, 938, 882] },
  "windows-network": { face: [93.45, 210, 930.55, 552], point: [512, 381], extent: [86, 208, 938, 852] },
  "linux-internal": { face: [108, 236, 916, 690.28], point: [512, 463.14], extent: [106, 234, 918, 792] },
  "linux-solid-state": { face: [156, 346, 788, 678], point: [472, 512], extent: [96, 312, 928, 712] },
  "linux-external": { face: [100, 184, 852, 598.92], point: [476, 391.46], extent: [98, 182, 937, 944] },
  "linux-removable": { face: [392, 330, 632, 800], point: [512, 565], extent: [356, 100, 668, 944] },
  "linux-card": { face: [272, 298, 752, 864], point: [512, 581], extent: [232, 108, 792, 904] },
  "linux-optical-drive": { face: [92, 300, 932, 594], point: [512, 447], extent: [90, 298, 934, 722] },
  "linux-optical": { face: [104, 104, 920, 920], point: [512, 228], extent: [88, 88, 936, 936] },
  "linux-server": { face: [124, 116, 900, 504], point: [512, 310], extent: [88, 114, 936, 900] },
  "linux-network": { face: [72, 276, 952, 872], point: [420, 560], extent: [72, 140, 952, 872] },
  "linux-multi-disk": { face: [112, 352, 824, 788.24], point: [468, 570.12], extent: [110, 134, 914, 886] },
};

/**
 * A drive's parts as the composer places things on them, the way `PartsDto::drive` reports them:
 * the face is the front, the back, the paper and the tab, and a disc says where on it a new layer
 * goes, since its middle is its hole.
 */
export function driveParts(id: string): Parts {
  const known = DRIVE_FACES[id] ?? DRIVE_FACES["mac-external"];
  const [x0, y0, x1, y1] = known.face;
  const [ax, ay] = known.point;
  const off = Math.abs(ax - (x0 + x1) / 2) > 1 || Math.abs(ay - (y0 + y1) / 2) > 1;
  return {
    canvas: 1024,
    folder: known.extent,
    front: known.face,
    front_radius: 24,
    front_step: null,
    anchor: off ? known.point : null,
    back: known.face,
    paper: known.face,
    tab: known.face,
  };
}
