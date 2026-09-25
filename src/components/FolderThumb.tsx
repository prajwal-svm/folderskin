import { useCallback, useEffect, useRef, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";
import type { Skin } from "../lib/tauri";
import { isYours } from "../lib/tags";
import { reducesMotion } from "../state/prefs";
import { StarIcon } from "./icons/star";
import { useT } from "../i18n";

/** Degrees the folder turns when the pointer is at the tile's edge. */
const TURN_Y = 12;
const TURN_X = 10;
/** Pixels the folder drifts toward the pointer, and how much it lifts while hovered. */
const DRIFT = 5;
const LIFT = 1.04;
/** Share of the remaining distance covered each frame: the folder eases after the pointer. */
const FOLLOW = 0.16;

/**
 * One folder in the library. While hovered it lifts slightly and turns toward the pointer: the
 * side under the pointer dips away, as if pressed. The motion eases after the pointer each frame
 * (no CSS transition to fight), and settles flat again when the pointer leaves. The star
 * springs in on hover and stays while the skin is a favourite.
 *
 * Every skin also has a ⋯ button, top left, for its menu: name, tags, what
 * is known about them, sharing and deleting. Return or F2 opens it from the keyboard, and
 * Delete or Backspace asks to delete the skin.
 */
export function FolderThumb({
  skin,
  index,
  selected,
  favorite,
  onSelect,
  onToggleFavorite,
  onRemove,
  onMenu,
  menuOpen = false,
}: {
  skin: Skin;
  index: number;
  selected: boolean;
  favorite: boolean;
  onSelect: () => void;
  onToggleFavorite: () => void;
  /** Asks to delete the skin. */
  onRemove?: () => void;
  /** Opens its menu beside `anchor`. */
  onMenu?: (anchor: HTMLElement, fromKeyboard: boolean) => void;
  /** Its menu is open, so the ⋯ button stays in view. */
  menuOpen?: boolean;
}) {
  const t = useT();
  const art = useRef<HTMLSpanElement>(null);
  const more = useRef<HTMLButtonElement>(null);
  // x and y run from -1 (left, top) to 1 (right, bottom); t* are where the pointer wants them.
  const m = useRef({ x: 0, y: 0, s: 1, tx: 0, ty: 0, ts: 1, frame: 0 });

  const step = useCallback(() => {
    const v = m.current;
    v.x += (v.tx - v.x) * FOLLOW;
    v.y += (v.ty - v.y) * FOLLOW;
    v.s += (v.ts - v.s) * FOLLOW;
    const rest = Math.abs(v.tx - v.x) < 0.002 && Math.abs(v.ty - v.y) < 0.002 && Math.abs(v.ts - v.s) < 0.0005;
    const el = art.current;
    if (rest) {
      v.frame = 0;
      v.x = v.tx;
      v.y = v.ty;
      v.s = v.ts;
    } else {
      v.frame = requestAnimationFrame(step);
    }
    if (!el) return;
    el.style.transform =
      rest && v.x === 0 && v.y === 0 && v.s === 1
        ? ""
        : `perspective(640px) rotateX(${(-v.y * TURN_X).toFixed(2)}deg) rotateY(${(v.x * TURN_Y).toFixed(2)}deg) ` +
          `translate3d(${(v.x * DRIFT).toFixed(2)}px, ${(v.y * DRIFT).toFixed(2)}px, 0) scale(${v.s.toFixed(4)})`;
  }, []);

  const aim = useCallback(
    (tx: number, ty: number, ts: number) => {
      const v = m.current;
      v.tx = tx;
      v.ty = ty;
      v.ts = ts;
      if (!v.frame) v.frame = requestAnimationFrame(step);
    },
    [step],
  );

  useEffect(() => () => cancelAnimationFrame(m.current.frame), []);

  const track = useCallback(
    (e: PointerEvent<HTMLButtonElement>) => {
      if (reducesMotion()) return;
      const r = e.currentTarget.getBoundingClientRect();
      const x = Math.min(1, Math.max(-1, ((e.clientX - r.left) / r.width) * 2 - 1));
      const y = Math.min(1, Math.max(-1, ((e.clientY - r.top) / r.height) * 2 - 1));
      aim(x, y, LIFT);
    },
    [aim],
  );

  const release = useCallback(() => aim(0, 0, 1), [aim]);

  const keys = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (onMenu && (e.key === "F2" || (e.key === "Enter" && selected))) {
      e.preventDefault();
      if (!selected) onSelect();
      // F2 renames, but a pack's skin keeps the name it was shared under: open the menu as a click does.
      onMenu(more.current ?? e.currentTarget, isYours(skin));
    } else if (onRemove && (e.key === "Delete" || e.key === "Backspace")) {
      e.preventDefault();
      onRemove();
    }
  };

  const cls = ["tile", selected ? "is-selected" : "", favorite ? "is-favorite" : "", menuOpen ? "is-menu-open" : ""]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={cls} style={{ "--i": Math.min(index, 24) } as CSSProperties}>
      <button
        type="button"
        className="tile-hit"
        aria-pressed={selected}
        aria-label={skin.source === "ai" ? t("library.tile.madeWithAi", { name: skin.name }) : skin.custom ? t("library.tile.yours", { name: skin.name }) : skin.name}
        aria-keyshortcuts={onMenu ? "F2" : undefined}
        onMouseDown={(e) => e.preventDefault()}
        onPointerMove={track}
        onPointerLeave={release}
        onClick={onSelect}
        onKeyDown={keys}
      >
        <span className="tile-art" ref={art}>
          <img className="tile-img" src={skin.thumbnail} alt="" draggable={false} loading="lazy" decoding="async" />
        </span>
        <span
          className="tile-name"
          data-tip={skin.name} data-tip-overflow
          // A double click on the name renames it, as in Finder: the menu opens with the name ready to
          // type over. Not for a pack's skin, whose name isn't the user's to change.
          onDoubleClick={onMenu && isYours(skin) ? () => more.current && onMenu(more.current, true) : undefined}
        >
          <span className="tile-name-text">{skin.name}</span>
        </span>
      </button>
      <button
        type="button"
        className="tile-star"
        aria-label={favorite ? t("library.tile.unfaveLabel", { name: skin.name }) : t("library.tile.faveLabel", { name: skin.name })}
        aria-pressed={favorite}
        data-tip={favorite ? t("library.tile.unfave") : t("library.tile.fave")}
        onMouseDown={(e) => e.preventDefault()}
        onClick={(e) => {
          e.stopPropagation();
          onToggleFavorite();
        }}
      >
        <StarIcon size={14} filled={favorite} />
      </button>
      {onMenu && (
        <button
          type="button"
          ref={more}
          className="tile-more"
          aria-label={t("library.tile.optionsLabel", { name: skin.name })}
          aria-haspopup="dialog"
          aria-expanded={menuOpen}
          data-tip={isYours(skin) ? t("library.tile.optionsYours") : t("library.tile.options")}
          onMouseDown={(e) => e.preventDefault()}
          onClick={(e) => {
            e.stopPropagation();
            onMenu(e.currentTarget, false);
          }}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
            <circle cx="5" cy="12" r="2" />
            <circle cx="12" cy="12" r="2" />
            <circle cx="19" cy="12" r="2" />
          </svg>
        </button>
      )}
    </div>
  );
}
