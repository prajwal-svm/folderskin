import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { Skin } from "../lib/tauri";
import { skinFacts } from "../lib/facts";
import { cleanName, MAX_NAME_CHARS } from "../lib/names";
import { isYours } from "../lib/tags";
import { TagInput } from "./TagInput";
import { DeleteIcon } from "./icons/delete";
import { EarthIcon } from "./icons/earth";
import { PaletteIcon } from "./icons/palette";
import { PencilIcon } from "./icons/pencil";
import { branded } from "./Brand";
import { useT } from "../i18n";

/** Wide enough for a whole name, tags and facts without cutting any of them short. */
const WIDTH = 360;
const MARGIN = 12;

const widthNow = () => Math.min(WIDTH, window.innerWidth - 2 * MARGIN);

/**
 * Everything about one skin in the library, opened from its ⋯ button (or F2, or a double click
 * on its name): its name (the user's own skins only) and tags, which save as they change, what is known about it (the model and prompt behind an AI result, or the
 * pack and person behind a community skin), and sharing and deleting it.
 *
 * It floats in its own layer, centred on the skin it's about, below the button that opened it or
 * above when there is no room below, and closes on Escape, a click elsewhere or a scroll
 * underneath. Nothing in it is cut short: a long name or prompt wraps instead.
 */
export function SkinMenu({
  skin,
  anchor,
  focusName,
  suggestions,
  onSave,
  onShare,
  onDesign,
  onDelete,
  onClose,
}: {
  skin: Skin;
  /** The button it opened from. Clicking that button again is not "elsewhere". */
  anchor: HTMLElement;
  /** Opened from the keyboard to rename: the name field takes the focus. */
  focusName: boolean;
  /** Tags the library already uses. */
  suggestions: string[];
  onSave: (name: string, tags: string[]) => void;
  /** Present for the user's own skins. */
  onShare?: () => void;
  /** Opens it in the composer: a design to edit again, or any other skin to remix. */
  onDesign: () => void;
  onDelete: () => void;
  onClose: () => void;
}) {
  const t = useT();
  const panel = useRef<HTMLDivElement>(null);
  const nameField = useRef<HTMLTextAreaElement>(null);
  const nameId = useId();
  const [pos, setPos] = useState<{ left: number; top: number; above: boolean; width: number } | null>(null);
  const canRename = isYours(skin);
  const [name, setName] = useState(skin.name);
  const [tags, setTags] = useState(skin.tags);
  /** The name as last saved, so Return and then leaving the field don't save it twice. */
  const savedName = useRef(skin.name);
  /** Saves a name typed but not yet saved; the listeners below call the latest one. */
  const flush = useRef(() => {});

  useLayoutEffect(() => {
    const place = () => {
      const a = anchor.getBoundingClientRect();
      // Centred on the skin (its tile, or its card in the assistant), not hung off the ⋯ in its corner.
      const on = (anchor.closest(".tile, .turn") ?? anchor).getBoundingClientRect();
      const width = widthNow();
      const height = panel.current?.offsetHeight ?? 0;
      const left = Math.min(Math.max(MARGIN, on.left + on.width / 2 - width / 2), window.innerWidth - width - MARGIN);
      const below = a.bottom + 6;
      const fits = below + height <= window.innerHeight - MARGIN;
      setPos({ left, top: fits ? below : Math.max(MARGIN, a.top - 6 - height), above: !fits, width });
    };
    place();
    // It grows as tags are added, which can push it past the bottom of the window.
    const grow = new ResizeObserver(place);
    if (panel.current) grow.observe(panel.current);
    window.addEventListener("resize", place);
    return () => {
      grow.disconnect();
      window.removeEventListener("resize", place);
    };
  }, [anchor]);

  // Focus waits until it's placed: before that it's hidden, and hidden things can't take focus.
  const placed = pos !== null;
  useEffect(() => {
    if (!placed) return;
    if (focusName && canRename) {
      nameField.current?.focus();
      nameField.current?.select();
    } else {
      panel.current?.focus({ preventScroll: true });
    }
  }, [focusName, placed]);

  useEffect(() => {
    // Whatever is half typed is kept: the name is saved, and blurring the tag field adds its tag.
    const leave = () => {
      flush.current();
      const active = document.activeElement;
      if (active instanceof HTMLElement && panel.current?.contains(active)) active.blur();
      onClose();
    };
    const down = (e: MouseEvent) => {
      const target = e.target as Node;
      if (!panel.current?.contains(target) && !anchor.contains(target)) leave();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      onClose();
      anchor.focus({ preventScroll: true });
    };
    const scroll = (e: Event) => {
      if (!panel.current?.contains(e.target as Node)) leave();
    };
    document.addEventListener("mousedown", down);
    document.addEventListener("keydown", key);
    window.addEventListener("scroll", scroll, true);
    return () => {
      document.removeEventListener("mousedown", down);
      document.removeEventListener("keydown", key);
      window.removeEventListener("scroll", scroll, true);
    };
  }, [anchor, onClose]);

  const saveName = () => {
    const clean = cleanName(name);
    if (!clean) setName(savedName.current);
    else if (clean !== savedName.current) {
      savedName.current = clean;
      onSave(clean, tags);
    }
  };
  useLayoutEffect(() => {
    flush.current = saveName;
  });

  // The name field grows with the name as it wraps, so a long one is shown whole.
  useLayoutEffect(() => {
    const el = nameField.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }, [name, pos?.width]);

  const saveTags = (next: string[]) => {
    setTags(next);
    const clean = cleanName(name);
    if (clean) savedName.current = clean;
    onSave(savedName.current, next);
  };

  const facts = skinFacts(skin);

  return createPortal(
    <div
      className={pos?.above ? "skin-menu is-above" : "skin-menu"}
      role="dialog"
      aria-label={t("library.menu.label", { name: skin.name })}
      tabIndex={-1}
      ref={panel}
      style={pos ? { left: pos.left, top: pos.top, width: pos.width } : { visibility: "hidden", width: widthNow() }}
    >
      <div className="skin-menu-block">
        {canRename ? (
          <>
            <label className="skin-menu-label" htmlFor={nameId}>
              {t("library.menu.name")}
            </label>
            <div className="skin-menu-field">
              <textarea
                ref={nameField}
                id={nameId}
                className="skin-menu-name"
                value={name}
                rows={1}
                maxLength={MAX_NAME_CHARS}
                spellCheck={false}
                autoComplete="off"
                // A name is one line: pasted line breaks become spaces.
                onChange={(e) => setName(e.target.value.replace(/[\r\n]+/g, " "))}
                onBlur={saveName}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    saveName();
                    panel.current?.focus({ preventScroll: true });
                  }
                }}
              />
              <span className="skin-menu-pencil" aria-hidden="true">
                <PencilIcon size={13} />
              </span>
            </div>
          </>
        ) : (
          /* A pack's skin keeps the name it was shared under: it is how the pack lists it, and
             how anyone else who adds the pack sees it. Tags below are still the user's own. */
          <p className="skin-menu-title">{skin.name}</p>
        )}
      </div>
      <div className="skin-menu-block">
        <p className="skin-menu-label">{t("library.menu.tags")}</p>
        <TagInput value={tags} onChange={saveTags} suggestions={suggestions} label={t("library.menu.addTag")} />
      </div>
      {facts.length > 0 && (
        <dl className="skin-facts">
          {facts.map(([label, value]) => (
            <div className="skin-fact" key={label}>
              <dt>{label}</dt>
              <dd>{branded(value)}</dd>
            </div>
          ))}
        </dl>
      )}
      <div className="skin-menu-actions">
        <button type="button" className="menu-item" onClick={onDesign}>
          <PaletteIcon size={16} />
          {skin.source === "composer" ? t("library.menu.editDesign") : t("library.menu.remix")}
        </button>
        {onShare && (
          <button type="button" className="menu-item" onClick={onShare}>
            <EarthIcon size={16} />
            {t("library.menu.share")}
          </button>
        )}
        <button type="button" className="menu-item is-danger" onClick={onDelete}>
          <DeleteIcon size={16} />
          {t("library.delete.action")}
        </button>
      </div>
    </div>,
    document.body,
  );
}
