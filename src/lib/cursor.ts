/**
 * Keeps the macOS pointer in step with the stylesheet.
 *
 * Every button, tab, switch and skin asks for the hand in CSS (styles/base.css), and Chromium and
 * Safari both compute it there. The window shows it only if WebKit hands it to AppKit, and in a
 * Tauri window on macOS the arrow can stay put while the page underneath asks for the hand, most
 * often after the pointer has left the window and come back (tauri-apps/tauri#1526,
 * tauri-apps/wry#175). So on macOS the app also sets the window's own cursor, from the same
 * computed style, whenever what's under the pointer changes. Where WebKit already shows it, this
 * sets the same cursor again and nothing changes.
 *
 * What's under a pointer that stands still changes too (a "Choose" pill appears, a dialog opens,
 * a button goes busy, a list scrolls), so those are looked at as well, once a frame at most.
 */
import type { CursorIcon } from "@tauri-apps/api/window";
import { localOs } from "./platform";

/** CSS cursors as the window names them. */
const ICONS: Record<string, CursorIcon> = {
  default: "default",
  none: "default",
  "context-menu": "contextMenu",
  help: "help",
  pointer: "hand",
  progress: "progress",
  wait: "wait",
  cell: "cell",
  crosshair: "crosshair",
  text: "text",
  "vertical-text": "verticalText",
  alias: "alias",
  copy: "copy",
  move: "move",
  "no-drop": "noDrop",
  "not-allowed": "notAllowed",
  grab: "grab",
  grabbing: "grabbing",
  "all-scroll": "allScroll",
  "col-resize": "colResize",
  "row-resize": "rowResize",
  "n-resize": "nResize",
  "e-resize": "eResize",
  "s-resize": "sResize",
  "w-resize": "wResize",
  "ne-resize": "neResize",
  "nw-resize": "nwResize",
  "se-resize": "seResize",
  "sw-resize": "swResize",
  "ew-resize": "ewResize",
  "ns-resize": "nsResize",
  "nesw-resize": "neswResize",
  "nwse-resize": "nwseResize",
  "zoom-in": "zoomIn",
  "zoom-out": "zoomOut",
};

/**
 * The window's cursor for a computed CSS `cursor` value, or null for `auto`, which depends on
 * what's under the pointer. A custom image's fallback keyword is the one used, and anything
 * unknown is the arrow.
 */
export function cursorFor(css: string): CursorIcon | null {
  const keyword = (css.split(",").pop() ?? "").trim().replace(/^-webkit-/, "");
  if (keyword === "" || keyword === "auto") return null;
  return ICONS[keyword] ?? "default";
}

/** Inputs that take typed text. */
const TYPED = new Set(["text", "search", "email", "url", "tel", "password", "number"]);

/** What WebKit shows for `cursor: auto`: the text cursor over text you can type or select, the arrow elsewhere. */
function autoCursor(el: Element, x: number, y: number): CursorIcon {
  if (el instanceof HTMLTextAreaElement || (el instanceof HTMLInputElement && TYPED.has(el.type))) {
    return el.disabled ? "default" : "text";
  }
  if (el instanceof HTMLElement && el.isContentEditable) return "text";
  const style = getComputedStyle(el);
  const select = style.getPropertyValue("-webkit-user-select") || style.getPropertyValue("user-select");
  if (select === "none") return "default";
  return onText(x, y) ? "text" : "default";
}

/** Whether the point is on a line of text rather than the space around it. */
function onText(x: number, y: number): boolean {
  const node = document.caretRangeFromPoint?.(x, y)?.startContainer;
  if (!node || node.nodeType !== Node.TEXT_NODE || !node.textContent?.trim()) return false;
  const lines = document.createRange();
  lines.selectNodeContents(node);
  for (const r of lines.getClientRects()) {
    if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) return true;
  }
  return false;
}

/** Starts following the pointer on macOS inside the app (not in the browser preview). Returns a stop. */
export function followCursor(): () => void {
  const inApp = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  if (!inApp || localOs() !== "macos") return () => {};
  let shown: CursorIcon = "default";
  let apply: ((icon: CursorIcon) => void) | null = null;
  void import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
    const win = getCurrentWindow();
    apply = (icon) => void win.setCursorIcon(icon).catch(() => {});
  });
  let x = -1;
  let y = -1;
  let queued = false;
  const look = () => {
    queued = false;
    if (x < 0 || !apply) return;
    const el = document.elementFromPoint(x, y);
    const icon = el ? (cursorFor(getComputedStyle(el).cursor) ?? autoCursor(el, x, y)) : "default";
    if (icon === shown) return;
    shown = icon;
    apply(icon);
  };
  const soon = () => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(look);
  };
  const moved = (e: PointerEvent) => {
    x = e.clientX;
    y = e.clientY;
    soon();
  };
  // Out of the window the cursor isn't the app's to set.
  const left = () => {
    x = -1;
  };
  const root = document.documentElement;
  document.addEventListener("pointermove", moved, { passive: true });
  document.addEventListener("pointerover", moved, { passive: true });
  root.addEventListener("pointerleave", left, { passive: true });
  // Scrolling, and a transition or animation coming to rest, move the page under a still pointer.
  const settled = { capture: true, passive: true } as const;
  document.addEventListener("scroll", soon, settled);
  document.addEventListener("transitionend", soon, settled);
  document.addEventListener("animationend", soon, settled);
  // Class, disabled and busy changes decide the cursor; inline styles change every frame of an
  // animation and never do, so they aren't watched.
  const changes = new MutationObserver(soon);
  changes.observe(root, {
    subtree: true,
    childList: true,
    attributes: true,
    attributeFilter: ["class", "disabled", "aria-disabled", "aria-busy", "role"],
  });
  return () => {
    document.removeEventListener("pointermove", moved);
    document.removeEventListener("pointerover", moved);
    root.removeEventListener("pointerleave", left);
    document.removeEventListener("scroll", soon, settled);
    document.removeEventListener("transitionend", soon, settled);
    document.removeEventListener("animationend", soon, settled);
    changes.disconnect();
  };
}
