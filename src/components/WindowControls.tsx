/**
 * The window's own minimise, maximise and close buttons, for Windows only.
 *
 * `src-tauri/src/window.rs` builds the Windows window undecorated, so there is no system caption
 * bar to sit above the islands. These take its place inside the folder island, top right, where
 * Windows users look for them. macOS keeps its traffic lights (the window is an overlay title bar
 * there) and Linux keeps its own decorations, so nothing renders on either.
 *
 * The glyphs are drawn at Windows' own metrics — a 10 px box, hairline strokes, square buttons —
 * rather than as the app's rounded icons, because a window control that doesn't look like the
 * system's reads as a mystery button.
 */

import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

const GLYPH = { width: 10, height: 10, viewBox: "0 0 10 10", "aria-hidden": true, focusable: false } as const;

export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    let live = true;
    const sync = () => void win.isMaximized().then((m) => live && setMaximized(m)).catch(() => {});
    sync();
    // Snapping and double-clicking the drag strip maximise without going through these buttons.
    const unlisten = win.onResized(sync);
    return () => {
      live = false;
      void unlisten.then((off) => off()).catch(() => {});
    };
  }, []);

  const win = () => getCurrentWindow();
  return (
    <div className="winctl" role="group" aria-label="window">
      <button type="button" className="winctl-btn" aria-label="Minimise" title="Minimise" onClick={() => void win().minimize()}>
        <svg {...GLYPH}>
          <path d="M0 5.5h10" stroke="currentColor" strokeWidth="1" />
        </svg>
      </button>
      <button
        type="button"
        className="winctl-btn"
        aria-label={maximized ? "Restore" : "Maximise"}
        title={maximized ? "Restore" : "Maximise"}
        onClick={() => void win().toggleMaximize()}
      >
        {maximized ? (
          <svg {...GLYPH}>
            <path d="M2.5 2.5V0.5h7v7h-2" fill="none" stroke="currentColor" strokeWidth="1" />
            <rect x="0.5" y="2.5" width="7" height="7" fill="none" stroke="currentColor" strokeWidth="1" />
          </svg>
        ) : (
          <svg {...GLYPH}>
            <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" strokeWidth="1" />
          </svg>
        )}
      </button>
      <button type="button" className="winctl-btn is-close" aria-label="Close" title="Close" onClick={() => void win().close()}>
        <svg {...GLYPH}>
          <path d="M0.5 0.5l9 9M9.5 0.5l-9 9" stroke="currentColor" strokeWidth="1" />
        </svg>
      </button>
    </div>
  );
}
