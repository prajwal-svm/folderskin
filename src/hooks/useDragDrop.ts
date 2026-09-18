import { useEffect, useRef } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { isTauri } from "../lib/devMock";

/**
 * Subscribes to Tauri's native drag-and-drop events (the only way to learn a dropped
 * file's path from a webview). `onHover` fires only when the hover state changes.
 */
export function useDragDrop(onDrop: (paths: string[]) => void, onHover: (hover: boolean) => void) {
  const hovering = useRef(false);
  const latest = useRef({ onDrop, onHover });
  latest.current = { onDrop, onHover };

  useEffect(() => {
    if (!isTauri()) return; // plain browser preview: no native drag-and-drop
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    const setHover = (value: boolean) => {
      if (hovering.current === value) return;
      hovering.current = value;
      latest.current.onHover(value);
    };
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === "enter" || payload.type === "over") setHover(true);
        else if (payload.type === "leave") setHover(false);
        else if (payload.type === "drop") {
          setHover(false);
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
