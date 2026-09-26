/**
 * System notifications, for a run over a folder's tree that ends while FolderSkin's window isn't
 * the one in front (tauri-plugin-notification). Permission is asked for the first time a run
 * starts, when a notification could be wanted, rather than as the app opens. On the desktop the
 * plugin always has it, and the system's own notification settings decide what shows.
 */
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** Asked already this session: a "no" isn't asked again. */
let asked = false;

/** Whether notifications may be shown, asking the first time if nobody has said yet. */
export async function askToNotify(): Promise<boolean> {
  if (await isPermissionGranted()) return true;
  if (asked) return false;
  asked = true;
  return (await requestPermission()) === "granted";
}

/** Whether FolderSkin's window is the one in front. */
export async function windowFocused(): Promise<boolean> {
  return getCurrentWindow().isFocused();
}

/** Shows `title`, and `body` under it when there is one, as a system notification, if allowed. */
export async function notify(title: string, body: string): Promise<void> {
  if (!(await isPermissionGranted())) return;
  sendNotification(body ? { title, body } : { title });
}
