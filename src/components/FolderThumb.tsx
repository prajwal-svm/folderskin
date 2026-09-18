import { useCallback, useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type MouseEvent, type PointerEvent } from "react";
import type { Skin } from "../lib/tauri";
import { NameField } from "./NameField";
import { PencilIcon } from "./icons/pencil";
import { SparklesIcon } from "./icons/sparkles";
import { StarIcon } from "./icons/star";

/** Degrees the folder turns when the pointer is at the tile's edge. */
const TURN_Y = 12;
const TURN_X = 10;
/** Pixels the folder drifts toward the pointer, and how much it lifts while hovered. */
const DRIFT = 5;
const LIFT = 1.04;
/** Share of the remaining distance covered each frame: the folder eases after the pointer. */
const FOLLOW = 0.16;

const reducedMotion = () => typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * One folder in the library. While hovered it lifts slightly and turns toward the pointer: the
 * side under the pointer dips away, as if pressed. The motion eases after the pointer each frame
 * (no CSS transition to fight), and settles flat again when the pointer leaves. The star
 * springs in on hover and stays while the skin is a favourite; skins the AI assistant made show
 * an "AI" badge on hover.
 *
 * The user's own skins can be renamed the way Finder does it: click the name of the selected
 * skin, or press Return or F2. Delete or Backspace asks to delete it.
 */
export function FolderThumb({
  skin,
  index,
  selected,
  favorite,
  onSelect,
  onToggleFavorite,
  onRemove,
  onRename,
}: {
  skin: Skin;
  index: number;
  selected: boolean;
  favorite: boolean;
  onSelect: () => void;
  onToggleFavorite: () => void;
  /** Asks to delete the skin. Present for the user's own skins. */
  onRemove?: () => void;
  /** Saves a new name. Present for the user's own skins. */
  onRename?: (name: string) => void;
}) {
  const art = useRef<HTMLSpanElement>(null);
  const hit = useRef<HTMLButtonElement>(null);
  const [renaming, setRenaming] = useState(false);
  /** Set for a moment after a rename, so the new name bumps in. */
  const [renamed, setRenamed] = useState(false);
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
      if (reducedMotion()) return;
      const r = e.currentTarget.getBoundingClientRect();
      const x = Math.min(1, Math.max(-1, ((e.clientX - r.left) / r.width) * 2 - 1));
      const y = Math.min(1, Math.max(-1, ((e.clientY - r.top) / r.height) * 2 - 1));
      aim(x, y, LIFT);
    },
    [aim],
  );

  const release = useCallback(() => aim(0, 0, 1), [aim]);

  useEffect(() => {
    if (!renamed) return;
    const t = window.setTimeout(() => setRenamed(false), 600);
    return () => window.clearTimeout(t);
  }, [renamed]);

  const ai = skin.source === "ai";
  const canRename = Boolean(onRename) && selected;

  // A click on the selected skin's name renames it (a double click on any name does the same:
  // the first click selects). Anywhere else, a click selects.
  const click = (e: MouseEvent<HTMLButtonElement>) => {
    if (canRename && e.detail > 0 && e.target instanceof Element && e.target.closest(".tile-name")) setRenaming(true);
    else onSelect();
  };

  const keys = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (onRename && (e.key === "F2" || (e.key === "Enter" && selected))) {
      e.preventDefault();
      if (!selected) onSelect();
      setRenaming(true);
    } else if (onRemove && (e.key === "Delete" || e.key === "Backspace")) {
      e.preventDefault();
      onRemove();
    }
  };

  const cls = [
    "tile",
    selected ? "is-selected" : "",
    favorite ? "is-favorite" : "",
    renaming ? "is-renaming" : "",
    renamed ? "is-renamed" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={cls} style={{ "--i": Math.min(index, 24) } as CSSProperties}>
      <button
        type="button"
        className="tile-hit"
        ref={hit}
        aria-pressed={selected}
        aria-label={ai ? `${skin.name} (made with AI)` : skin.custom ? `${skin.name} (yours)` : skin.name}
        aria-keyshortcuts={onRename ? "F2" : undefined}
        onMouseDown={(e) => e.preventDefault()}
        onPointerMove={track}
        onPointerLeave={release}
        onClick={click}
        onKeyDown={keys}
      >
        <span className="tile-art" ref={art}>
          <img className="tile-img" src={skin.thumbnail} alt="" draggable={false} />
        </span>
        {ai && (
          <span className="tile-badge" title="Made with AI">
            <SparklesIcon size={12} />
            AI
          </span>
        )}
        <span className="tile-name" title={canRename ? "Rename" : undefined}>
          <span className="tile-name-text">{skin.name}</span>
          {canRename && <PencilIcon size={11} className="tile-name-pencil" />}
        </span>
      </button>
      {renaming && onRename && (
        <NameField
          value={skin.name}
          label={`new name for ${skin.name}`}
          className="tile-name-field"
          onRename={(name) => {
            onRename(name);
            setRenamed(true);
          }}
          onClose={(how) => {
            setRenaming(false);
            // Back to the tile after the keyboard ends it; a click elsewhere keeps its own focus.
            if (how !== "away") hit.current?.focus({ preventScroll: true });
          }}
        />
      )}
      <button
        type="button"
        className="tile-star"
        aria-label={favorite ? `remove ${skin.name} from favourites` : `add ${skin.name} to favourites`}
        aria-pressed={favorite}
        title={favorite ? "Remove from favourites" : "Add to favourites"}
        onMouseDown={(e) => e.preventDefault()}
        onClick={(e) => {
          e.stopPropagation();
          onToggleFavorite();
        }}
      >
        <StarIcon size={14} filled={favorite} />
      </button>
      {onRemove && (
        <button
          type="button"
          className="tile-remove"
          aria-label={`delete ${skin.name}`}
          title="Delete…"
          onMouseDown={(e) => e.preventDefault()}
          onClick={(e) => {
            e.stopPropagation();
            onRemove();
          }}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" aria-hidden="true">
            <path d="M18 6 6 18M6 6l12 12" />
          </svg>
        </button>
      )}
    </div>
  );
}
