/**
 * Where the folder's parts are, in canvas units. Kept apart from the design document so the
 * browser preview's stand-ins can use it without loading the composer.
 */

/**
 * The folder's parts in canvas units, as `composer_template` reports them from the Rust
 * geometry. Only used to place things (a new layer lands in the middle of the front panel);
 * the folder itself is never drawn from these. A drive (`composer_drive_template`) reports its
 * face as the front, the back, the paper and the tab alike.
 */
export type Parts = {
  canvas: number;
  folder: [number, number, number, number];
  front: [number, number, number, number];
  front_radius: number;
  /** The corner of `front` that isn't front: under Windows' tab, where its front starts lower. Null on the Mac's folder. */
  front_step: [number, number, number, number] | null;
  /** Where a new layer goes when that isn't the middle of `front`: on a disc, whose middle is its hole. */
  anchor?: [number, number] | null;
  back: [number, number, number, number];
  paper: [number, number, number, number];
  tab: [number, number, number, number];
};

/** Which folder a design is drawn on: FolderSkin's own, as Finder shows it, Windows' or Linux's. */
export type FolderStyle = "mac" | "windows" | "linux";

/** Every folder, in the order they're offered. */
export const FOLDER_STYLES: FolderStyle[] = ["mac", "windows", "linux"];

export const isFolderStyle = (v: unknown): v is FolderStyle => v === "mac" || v === "windows" || v === "linux";

/** The same numbers as `crates/folderskin-core/src/geometry.rs`, for tests and the browser preview. */
export const FALLBACK_PARTS: Parts = {
  canvas: 1024,
  folder: [15, 36.5, 1009, 973.5],
  front: [15, 160.5, 1009, 973.5],
  front_radius: 55,
  front_step: null,
  back: [29, 36.5, 995, 973.5],
  paper: [74.5, 131.3, 949.5, 973.5],
  tab: [61, 36.5, 441.7, 97],
};

/** Windows' folder, the same numbers as `crates/folderskin-core/src/geometry_windows.rs`. It has no paper: `paper` is where its back panel's body shows. */
export const WINDOWS_PARTS: Parts = {
  canvas: 1024,
  folder: [64, 136, 960, 840],
  front: [64, 248, 960, 840],
  front_radius: 36,
  front_step: [64, 248, 464, 296],
  back: [64, 136, 960, 840],
  paper: [64, 232, 960, 840],
  tab: [64, 136, 464, 232],
};

/** Linux's folder, the same numbers as `crates/folderskin-core/src/geometry_linux.rs`. Like Windows', it has no paper. */
export const LINUX_PARTS: Parts = {
  canvas: 1024,
  folder: [72, 140, 952, 872],
  front: [72, 276, 952, 872],
  front_radius: 40,
  front_step: null,
  back: [72, 140, 952, 872],
  paper: [72, 206, 952, 872],
  tab: [72, 140, 447.9, 206],
};

export const fallbackParts = (style: FolderStyle): Parts => (style === "windows" ? WINDOWS_PARTS : style === "linux" ? LINUX_PARTS : FALLBACK_PARTS);

export const centreOf = ([x0, y0, x1, y1]: [number, number, number, number]) => ({ x: (x0 + x1) / 2, y: (y0 + y1) / 2 });

/** Where a new layer goes: the middle of the front, or on a disc a point on it that shows. */
export const anchorOf = (parts: Parts) => (parts.anchor ? { x: parts.anchor[0], y: parts.anchor[1] } : centreOf(parts.front));
