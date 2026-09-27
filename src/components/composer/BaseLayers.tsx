import "../../i18n/composer";
import { t, useLocale } from "../../i18n";
import { useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import { BASE_ID, baseSelection, partLabel, partSelection, type BasePart } from "../../composer/base";
import { cssColor } from "../../composer/color";
import { EyeOffIcon, EyeOpenIcon, FolderIcon, TrashIcon, UndoIcon, ChevronDownIcon, ChevronRightIcon } from "../icons/composer";
import { HardDriveIcon } from "../icons/hard-drive";

/** A part of the folder or drive as its row shows it. */
export type BaseRow = BasePart & {
  /** The colour its swatch shows: its own, the one it was given, or the design's on it. Null for none to show. */
  color: string | null;
  hidden: boolean;
};

/**
 * The folder or drive under the design, at the foot of the layers list, as a group of its own:
 * its row picks the whole of it (to move, turn or size it on the canvas), and under it its parts,
 * top first, each with its colour, to pick, hide or remove. Removed parts leave the list; the
 * group's row brings them back.
 */
export function BaseLayers({
  name,
  drive,
  rows,
  removed,
  selectedId,
  onSelect,
  onToggle,
  onRemove,
  onRestore,
}: {
  /** What the folder or drive is: "USB stick", "Mac folder". */
  name: string;
  drive: boolean;
  /** The parts still in it, top first. */
  rows: BaseRow[];
  /** How many parts were removed. */
  removed: number;
  selectedId: string | null;
  onSelect: (id: string) => void;
  onToggle: (part: string) => void;
  onRemove: (part: string) => void;
  onRestore: () => void;
}) {
  useLocale();
  const [open, setOpen] = useState(true);
  const list = useRef<HTMLDivElement>(null);
  const picked = baseSelection(selectedId);
  const ids = [BASE_ID, ...(open ? rows.map((r) => partSelection(r.id)) : [])];
  const current = selectedId && ids.includes(selectedId) ? selectedId : null;

  /** Arrow keys move through the group's rows, Enter and Space pick one. */
  const keys = (e: KeyboardEvent<HTMLDivElement>, id: string) => {
    if (e.target !== e.currentTarget) return;
    const step = e.key === "ArrowUp" ? -1 : e.key === "ArrowDown" ? 1 : 0;
    if (step) {
      e.preventDefault();
      const next = ids[ids.indexOf(id) + step];
      if (!next) return;
      onSelect(next);
      window.requestAnimationFrame(() => list.current?.querySelector<HTMLElement>(".cmp-base-row.is-on")?.focus());
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onSelect(id);
    } else if (e.key === "ArrowLeft" && open && id === BASE_ID) {
      e.preventDefault();
      setOpen(false);
    } else if (e.key === "ArrowRight" && !open && id === BASE_ID) {
      e.preventDefault();
      setOpen(true);
    }
  };

  const Glyph = drive ? HardDriveIcon : FolderIcon;
  return (
    <div className="cmp-base" ref={list} role="listbox" aria-label={drive ? t("composer.base.driveParts") : t("composer.base.folderParts")}>
      <div
        role="option"
        aria-label={name}
        aria-selected={picked?.part === null}
        aria-expanded={open}
        className={picked?.part === null ? "cmp-base-row is-group is-on" : "cmp-base-row is-group"}
        tabIndex={picked?.part === null || !current ? 0 : -1}
        onKeyDown={(e) => keys(e, BASE_ID)}
        onPointerDown={(e) => {
          if ((e.target as Element).closest("button")) return;
          onSelect(BASE_ID);
        }}
      >
        <button
          type="button"
          className="cmp-base-caret"
          aria-label={open ? t("composer.base.collapse") : t("composer.base.expand")}
          onClick={() => setOpen(!open)}
        >
          {open ? <ChevronDownIcon size={13} /> : <ChevronRightIcon size={13} />}
        </button>
        <span className="cmp-thumb is-icon" aria-hidden="true">
          <Glyph size={15} />
        </span>
        <span className="cmp-layer-name">
          <span className="cmp-base-label">{name}</span>
        </span>
        {removed > 0 && (
          <button
            type="button"
            className="cmp-layer-btn is-set is-restore"
            aria-label={t("composer.base.restoreLabel", { count: removed })}
            data-tip={t("composer.base.restore", { count: removed })}
            onClick={onRestore}
          >
            <UndoIcon size={13} />
          </button>
        )}
      </div>
      {open &&
        rows.map((row) => {
          const id = partSelection(row.id);
          const label = partLabel(row.id);
          const on = selectedId === id;
          return (
            <div
              key={row.id}
              role="option"
              aria-label={label}
              aria-selected={on}
              className={`cmp-base-row is-part${on ? " is-on" : ""}${row.hidden ? " is-hidden" : ""}`}
              tabIndex={on ? 0 : -1}
              onKeyDown={(e) => keys(e, id)}
              onPointerDown={(e) => {
                if ((e.target as Element).closest("button")) return;
                onSelect(id);
              }}
            >
              <span
                className={row.color ? "cmp-thumb is-paint is-part" : "cmp-thumb is-part is-design"}
                style={row.color ? ({ "--g": cssColor(row.color) } as CSSProperties) : undefined}
                aria-hidden="true"
              />
              <span className="cmp-layer-name">
                <span className="cmp-base-label">{label}</span>
              </span>
              <button
                type="button"
                className={row.hidden ? "cmp-layer-btn is-set" : "cmp-layer-btn"}
                aria-label={row.hidden ? t("composer.layers.showLabel", { name: label }) : t("composer.layers.hideLabel", { name: label })}
                data-tip={row.hidden ? t("composer.layers.show") : t("composer.layers.hide")}
                onClick={() => onToggle(row.id)}
              >
                {row.hidden ? <EyeOffIcon size={14} /> : <EyeOpenIcon size={14} />}
              </button>
              <button
                type="button"
                className="cmp-layer-btn is-danger"
                aria-label={t("composer.base.removeLabel", { name: label })}
                data-tip={t("composer.base.remove")}
                onClick={() => onRemove(row.id)}
              >
                <TrashIcon size={13} />
              </button>
            </div>
          );
        })}
    </div>
  );
}
