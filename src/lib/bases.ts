/**
 * The bases a skin can be drawn on, as the webview names them: FolderSkin's folders, every drive
 * shape, and none at all. The list itself comes from Rust (`api.baseShapes`, `src-tauri/src/bases.rs`),
 * each entry naming its message by key; this is where that key becomes words.
 */
import { t, type MessageKey } from "../i18n";

/** A base's name in the language on show, by its id: `folder-linux`, `drive-mac-external`, `free`. */
export const baseLabel = (id: string): string => t(`common.bases.${id}` as MessageKey);

/** A drive shape's name, by the drive's own id: `mac-external`. */
export const driveLabel = (drive: string): string => baseLabel(`drive-${drive}`);
