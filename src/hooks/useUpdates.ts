import { useCallback, useEffect, useRef, useState } from "react";
import { checksOnLaunch, findUpdate, type AvailableUpdate } from "../lib/updater";

export type UpdateStatus =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "current" }
  | { state: "available"; update: AvailableUpdate }
  | { state: "failed" };

/** The launch check waits this long, so it doesn't compete with the library loading. */
const LAUNCH_DELAY_MS = 3000;

/**
 * Whether a newer FolderSkin exists, and the dialog that installs it. A release build asks once
 * a few seconds after it opens and shows the dialog when there is one; About and Settings ask
 * again when you click, and answer in place.
 */
export function useUpdates() {
  const [status, setStatus] = useState<UpdateStatus>({ state: "idle" });
  const [dialog, setDialog] = useState<AvailableUpdate | null>(null);
  const asking = useRef(false);

  const ask = useCallback(async (showDialog: boolean) => {
    if (asking.current) return;
    asking.current = true;
    setStatus({ state: "checking" });
    try {
      const update = await findUpdate();
      setStatus(update ? { state: "available", update } : { state: "current" });
      if (update && showDialog) setDialog(update);
    } catch (e) {
      console.warn("folderskin: couldn't check for updates:", e);
      setStatus({ state: "failed" });
    } finally {
      asking.current = false;
    }
  }, []);

  useEffect(() => {
    if (!checksOnLaunch()) return;
    const timer = window.setTimeout(() => void ask(true), LAUNCH_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [ask]);

  const check = useCallback(() => void ask(false), [ask]);
  const showDialog = useCallback(() => {
    if (status.state === "available") setDialog(status.update);
  }, [status]);
  const hideDialog = useCallback(() => setDialog(null), []);

  return { status, check, dialog, showDialog, hideDialog };
}
