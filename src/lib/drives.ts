/**
 * Drives picked instead of folders (docs/DRIVES.md): what `inspect_path` says about one, and how
 * the stage and the library speak of it.
 */
import { t, type MessageKey } from "../i18n";
import type { Skin } from "./tauri";

/** The kinds of drive FolderSkin tells apart, by the ids docs/DRIVES.md gives them. */
export type DriveKind =
  | "startup"
  | "internal"
  | "solid-state"
  | "external"
  | "removable"
  | "card"
  | "optical"
  | "disk-image"
  | "network"
  | "time-machine"
  | "multi-disk";

/** Why FolderSkin can't change a drive's icon (`Refusal` in apply/drive.rs). */
export type DriveLock = "startup-sealed" | "startup-system" | "read-only";

/** A drive, as `inspect_path` describes it. */
export type Drive = {
  kind: DriveKind;
  /** The shape a skin goes on for it on this computer: `mac-external`. */
  shape: string;
  /** Its own name, or empty when it has none. */
  label: string;
  /** A Windows drive's letter. */
  letter: string | null;
  startup: boolean;
  read_only: boolean;
  network: boolean;
  /** Why FolderSkin can't change its icon, when it can't. */
  locked: DriveLock | null;
  /** Where each skin's picture on it is: this, then the skin's id. */
  thumbnails: string;
  /** The drive with nothing on it, which shows while a skin's picture on it is on its way. */
  plain: string;
};

/** What a drive of its kind is called: "External drive", or on Windows "System drive" for the one it runs from. */
export function driveKindName(drive: Pick<Drive, "kind" | "letter">): string {
  if (drive.kind === "startup" && drive.letter) return t("common.drives.system");
  return t(`common.drives.${drive.kind}` as MessageKey);
}

/**
 * A drive's name as its system shows it: its own ("Macintosh HD"), with its letter on Windows
 * ("Backup (E:)"), and for a drive with no name of its own what kind of drive it is ("USB drive (F:)").
 */
export function driveName(drive: Drive): string {
  const name = drive.label || driveKindName(drive);
  return drive.letter ? `${name} (${drive.letter}:)` : name;
}

/** True for a skin drawn on a drive: a finished drive, or artwork from a drive pack. */
export const isDriveSkin = (skin: Pick<Skin, "shape">) => skin.shape === "drive";

/**
 * The library in the order that suits what's picked: drive skins first while a drive is, and
 * after the folder skins otherwise. Each group keeps the order it had.
 */
export function shapeFirst<T extends Pick<Skin, "shape">>(skins: T[], drive: boolean): T[] {
  const drives = skins.filter(isDriveSkin);
  if (drives.length === 0 || drives.length === skins.length) return skins;
  const folders = skins.filter((s) => !isDriveSkin(s));
  return drive ? [...drives, ...folders] : [...folders, ...drives];
}

/**
 * True when a skin is drawn again for the drive it goes on: artwork is wrapped onto the drive's
 * shape, and a finished drive or folder is used as it was drawn.
 */
export const drawnOnDrive = (skin: Pick<Skin, "kind">) => skin.kind === "artwork";
