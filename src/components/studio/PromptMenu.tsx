import { useEffect, useLayoutEffect, useRef, useState, type FormEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import type { MenuGroup, MenuItem } from "../../lib/promptMenu";
import type { ShapeInfo } from "../../lib/shapes";
import { ShapeThumb } from "./ShapePicker";
import { PaletteIcon } from "../icons/palette";
import { SparklesIcon } from "../icons/sparkles";
import { PlusIcon, TickIcon, TypeIcon, XIcon } from "../icons/composer";
import { useT } from "../../i18n";

const MARGIN = 10;
const GAP = 6;
/** Tallest the menu grows, before it scrolls. */
const MAX_HEIGHT = 340;

/** Saving what's in the box as a prompt: the name being typed, and what to do with it. */
export type Naming = {
  name: string;
  onName: (name: string) => void;
  /** The name is taken: saving replaces that prompt. */
  taken: string | null;
  /** Someone the words name as their style, which is worth a word before saving. */
  named: string | null;
  /** The look saved with it, by name. */
  look: string | null;
  busy: boolean;
  onSave: () => void;
  /** Escape: back to the box, keeping the words. */
  onCancel: () => void;
  /** The focus went somewhere else altogether: the menu goes. */
  onLeave: () => void;
};

/**
 * The menu "@" or "/" opens over the prompt box: rows in groups, one of them active, which the keys
 * in the box move through and choose from (PromptBox.tsx) and the pointer can too. The box keeps the
 * focus, so typing goes on narrowing the rows; the screen reader hears the active row through the
 * box's `aria-activedescendant`. It sits above the box when there's room, and below it otherwise.
 */
export function PromptMenu({
  id,
  anchor,
  label,
  groups,
  active,
  onActive,
  onChoose,
  onRemove,
  shapes,
  empty,
  naming,
}: {
  id: string;
  /** The prompt box, which the menu lines up with. */
  anchor: HTMLElement;
  label: string;
  groups: MenuGroup[];
  active: number;
  onActive: (index: number) => void;
  onChoose: (item: MenuItem) => void;
  /** Removes one of the user's own prompts. */
  onRemove?: (item: MenuItem) => void;
  /** Every shape, for the pictures of the "@" menu's rows. */
  shapes: ShapeInfo[];
  /** What the menu says when nothing matches. */
  empty: string;
  /** Present while a prompt is being named, which the menu shows instead of its rows. */
  naming: Naming | null;
}) {
  const t = useT();
  const panel = useRef<HTMLDivElement>(null);
  const nameInput = useRef<HTMLInputElement>(null);
  const [pos, setPos] = useState<{ left: number; width: number; top?: number; bottom?: number; maxHeight: number } | null>(null);

  useLayoutEffect(() => {
    const place = () => {
      const a = anchor.getBoundingClientRect();
      const above = a.top - MARGIN - GAP;
      const below = window.innerHeight - a.bottom - MARGIN - GAP;
      const width = Math.min(a.width, window.innerWidth - 2 * MARGIN);
      const left = Math.min(Math.max(MARGIN, a.left), window.innerWidth - width - MARGIN);
      // Above the box, where the chat is, unless it's near the top and there's more room below.
      if (above >= Math.min(MAX_HEIGHT, 220) || above >= below) {
        setPos({ left, width, bottom: window.innerHeight - a.top + GAP, maxHeight: Math.min(MAX_HEIGHT, above) });
      } else {
        setPos({ left, width, top: a.bottom + GAP, maxHeight: Math.min(MAX_HEIGHT, below) });
      }
    };
    place();
    const grow = new ResizeObserver(place);
    grow.observe(anchor);
    window.addEventListener("resize", place);
    return () => {
      grow.disconnect();
      window.removeEventListener("resize", place);
    };
  }, [anchor]);

  // The active row stays in view as the keys move through a list longer than the menu. The first
  // row brings the top of the menu with it, so its heading shows too.
  useEffect(() => {
    const el = panel.current;
    if (!el) return;
    if (active === 0) el.scrollTop = 0;
    else el.querySelector(`#${CSS.escape(`${id}-${active}`)}`)?.scrollIntoView({ block: "nearest" });
  }, [id, active, groups, pos]);

  // Naming a prompt takes the focus into its field; the box gets it back when it's saved or not.
  const namingOpen = naming !== null;
  useEffect(() => {
    if (namingOpen) nameInput.current?.focus();
  }, [namingOpen]);

  let index = -1;
  const row = (item: MenuItem) => {
    index += 1;
    const i = index;
    const on = i === active;
    const current = (item.kind === "shape" || item.kind === "style") && item.current;
    return (
      <div
        key={`${item.kind}:${item.id}`}
        id={`${id}-${i}`}
        role="option"
        aria-selected={on}
        className={on ? "pm-row is-active" : "pm-row"}
        onMouseMove={() => !on && onActive(i)}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => onChoose(item)}
      >
        <span className="pm-glyph">{glyph(item, shapes)}</span>
        <span className="pm-text">
          <span className="pm-name">{item.name}</span>
          {item.note && <span className="pm-note">{item.note}</span>}
        </span>
        {current && (
          <span className="pm-current">
            <TickIcon size={14} />
            {item.kind === "style" && t("ai.slash.inUse")}
          </span>
        )}
        {item.kind === "prompt" && onRemove && (
          <button
            type="button"
            className="pm-remove"
            aria-label={t("ai.slash.removeLabel", { name: item.name })}
            data-tip={t("ai.slash.removeTip")}
            tabIndex={-1}
            onMouseDown={(e) => e.preventDefault()}
            onClick={(e) => {
              e.stopPropagation();
              onRemove(item);
            }}
          >
            <XIcon size={13} />
          </button>
        )}
      </div>
    );
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    // The menu is drawn outside the prompt box, but React still hands its events up to the box's
    // own form, whose submit sends the idea.
    e.stopPropagation();
    if (naming && naming.name.trim() && !naming.busy) naming.onSave();
  };

  return createPortal(
    <div
      ref={panel}
      className={`pm${pos?.top !== undefined ? " is-below" : ""}`}
      style={pos ? { left: pos.left, width: pos.width, top: pos.top, bottom: pos.bottom, maxHeight: pos.maxHeight } : { visibility: "hidden" }}
    >
      {naming ? (
        <form className="pm-naming" onSubmit={submit}>
          <label className="pm-naming-title" htmlFor={`${id}-name`}>
            {t("ai.slash.save")}
          </label>
          <div className="pm-naming-row">
            <input
              ref={nameInput}
              id={`${id}-name`}
              className="input pm-naming-input"
              value={naming.name}
              maxLength={60}
              placeholder={t("ai.slash.namePlaceholder")}
              aria-label={t("ai.slash.nameLabel")}
              spellCheck={false}
              onChange={(e) => naming.onName(e.target.value)}
              onBlur={(e) => {
                const to = e.relatedTarget as Element | null;
                if (!to?.closest(".pm") && !to?.closest(".composer")) naming.onLeave();
              }}
              onKeyDown={(e) => {
                if (e.key !== "Escape") return;
                e.preventDefault();
                e.stopPropagation();
                naming.onCancel();
              }}
            />
            <button type="submit" className="btn btn-primary btn-sm" disabled={!naming.name.trim() || naming.busy} aria-busy={naming.busy || undefined}>
              {naming.taken ? t("ai.slash.replaceAction") : naming.named ? t("ai.slash.saveAnyway") : t("ai.slash.saveAction")}
            </button>
          </div>
          {naming.look && <p className="pm-naming-note">{t("ai.slash.withLook", { look: naming.look })}</p>}
          {naming.taken && <p className="pm-naming-note">{t("ai.slash.replaceNote", { name: naming.taken })}</p>}
          {naming.named && <p className="pm-naming-note is-warn">{t("ai.slash.namesSomeone", { name: naming.named })}</p>}
        </form>
      ) : (
        <div className="pm-list" id={id} role="listbox" aria-label={label}>
          {groups.length === 0 && <p className="pm-empty">{empty}</p>}
          {groups.map((g) => (
            <div className="pm-group" key={g.id} role="group" aria-label={g.title || undefined}>
              {g.title && (
                <p className="pm-heading" aria-hidden="true">
                  {g.title}
                </p>
              )}
              {g.items.map(row)}
            </div>
          ))}
        </div>
      )}
    </div>,
    document.body,
  );
}

/** The picture at the start of a row: the shape itself, or a sign of what kind of row it is. */
function glyph(item: MenuItem, shapes: ShapeInfo[]): ReactNode {
  switch (item.kind) {
    case "shape":
      return <ShapeThumb shape={shapes.find((s) => s.id === item.id)} />;
    case "style":
      return <PaletteIcon size={15} />;
    case "idea":
      return <SparklesIcon size={15} />;
    case "prompt":
      return <TypeIcon size={15} />;
    case "save":
      return <PlusIcon size={15} />;
  }
}
