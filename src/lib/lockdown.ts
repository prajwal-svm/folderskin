/**
 * A release build is an app, not a web page. Right-click has no browser menu (Reload, Inspect
 * Element, Save Image), and the keys that would reload the page, open an inspector or show the
 * page's source do nothing. The inspector itself isn't built into release builds: tauri has no
 * `devtools` feature in src-tauri/Cargo.toml. Development builds keep all of it.
 */

type Keys = Pick<KeyboardEvent, "key" | "code" | "metaKey" | "ctrlKey" | "shiftKey" | "altKey">;

/** A shortcut that belongs to the browser inside the window rather than to FolderSkin. */
export function isBrowserShortcut(e: Keys): boolean {
  if (e.key === "F5" || e.key === "F12") return true;
  // By key position: with ⌥ held, macOS turns ⌘⌥I into ⌘ˆ.
  const letter = e.code.startsWith("Key") ? e.code.slice(3) : "";
  const mod = e.metaKey || e.ctrlKey;
  // reload, and reload without the cache
  if (mod && !e.altKey && letter === "R") return true;
  // the inspector, its console and its element picker: ⌘⌥ on macOS, Ctrl+Shift elsewhere
  if ((e.metaKey && e.altKey) || (e.ctrlKey && e.shiftKey)) return ["I", "J", "C"].includes(letter);
  // the page's source
  return e.ctrlKey && !e.shiftKey && !e.altKey && letter === "U";
}

export function lockDown(): void {
  document.addEventListener("contextmenu", (e) => e.preventDefault());
  // Capturing, so no handler in the page sees these keys first.
  window.addEventListener(
    "keydown",
    (e) => {
      if (isBrowserShortcut(e)) {
        e.preventDefault();
        e.stopPropagation();
      }
    },
    true,
  );
}
