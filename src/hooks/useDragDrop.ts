import { useEffect, useRef } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { isTauri } from "../lib/devMock";
import { dragInfoFor } from "../lib/files";
import type { DragInfo } from "../state/dropzone";

/**
 * Subscribes to Tauri's native drag-and-drop events (the only way to learn a dropped file's
 * path from a webview). `onDrag` reports what is being carried as soon as it enters the
 * window, so the UI can say "let go to pick Projects" before anything lands, and null when
 * it leaves or drops.
 */
export function useDragDrop(onDrop: (paths: string[]) => void, onDrag: (info: DragInfo | null) => void) {
  const current = useRef<DragInfo | null>(null);
  const latest = useRef({ onDrop, onDrag });
  latest.current = { onDrop, onDrag };

  useEffect(() => {
    if (!isTauri()) return; // plain browser preview: no native drag-and-drop
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    const report = (info: DragInfo | null) => {
      if (current.current?.kind === info?.kind && current.current?.name === info?.name) return;
      current.current = info;
      latest.current.onDrag(info);
    };
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === "enter") report(dragInfoFor(payload.paths) ?? { kind: "folder", name: "" });
        else if (payload.type === "leave") report(null);
        else if (payload.type === "drop") {
          report(null);
          latest.current.onDrop(payload.paths);
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => {
        /* not running inside Tauri (e.g. vite preview): drag-and-drop is unavailable */
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
}
