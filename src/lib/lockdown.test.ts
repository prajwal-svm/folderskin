import { describe, expect, it } from "vitest";
import { isBrowserShortcut } from "./lockdown";

const press = (key: string, code: string, mods: { meta?: boolean; ctrl?: boolean; shift?: boolean; alt?: boolean } = {}) => ({
  key,
  code,
  metaKey: !!mods.meta,
  ctrlKey: !!mods.ctrl,
  shiftKey: !!mods.shift,
  altKey: !!mods.alt,
});

describe("isBrowserShortcut", () => {
  it("catches reloading, the inspector and the page source on every platform", () => {
    for (const e of [
      press("F5", "F5"),
      press("F12", "F12"),
      press("r", "KeyR", { meta: true }),
      press("R", "KeyR", { meta: true, shift: true }),
      press("r", "KeyR", { ctrl: true }),
      // macOS types ˆ for ⌘⌥I, so the key's position decides
      press("ˆ", "KeyI", { meta: true, alt: true }),
      press("∆", "KeyJ", { meta: true, alt: true }),
      press("ç", "KeyC", { meta: true, alt: true }),
      press("I", "KeyI", { ctrl: true, shift: true }),
      press("J", "KeyJ", { ctrl: true, shift: true }),
      press("C", "KeyC", { ctrl: true, shift: true }),
      press("u", "KeyU", { ctrl: true }),
    ]) {
      expect(isBrowserShortcut(e), JSON.stringify(e)).toBe(true);
    }
  });

  it("leaves FolderSkin's own keys and editing alone", () => {
    for (const e of [
      press("f", "KeyF", { meta: true }),
      press("f", "KeyF", { ctrl: true }),
      press("c", "KeyC", { meta: true }),
      press("v", "KeyV", { meta: true }),
      press("c", "KeyC", { ctrl: true }),
      press("i", "KeyI", { meta: true }),
      press("r", "KeyR"),
      press("R", "KeyR", { shift: true }),
      press("Escape", "Escape"),
      press("Enter", "Enter"),
    ]) {
      expect(isBrowserShortcut(e), JSON.stringify(e)).toBe(false);
    }
  });
});
