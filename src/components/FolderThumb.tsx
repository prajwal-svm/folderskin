import { useCallback, useEffect, useRef, type CSSProperties, type PointerEvent } from "react";
import type { Skin } from "../lib/tauri";
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
 * springs in on hover and stays while the skin is a favourite.
 */
export function FolderThumb({
  skin,
  index,
  selected,
  favorite,
  onSelect,
  onToggleFavorite,
  onRemove,
}: {
  skin: Skin;
  index: number;
  selected: boolean;
  favorite: boolean;
  onSelect: () => void;
  onToggleFavorite: () => void;
  /** Present for the user's own skins. */
  onRemove?: () => void;
}) {
  const art = useRef<HTMLSpanElement>(null);
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

  const cls = ["tile", selected ? "is-selected" : "", favorite ? "is-favorite" : ""].filter(Boolean).join(" ");

  return (
    <div className={cls} style={{ "--i": Math.min(index, 24) } as CSSProperties}>
      <button
        type="button"
        className="tile-hit"
        aria-pressed={selected}
        aria-label={skin.custom ? `${skin.name} (yours)` : skin.name}
        onMouseDown={(e) => e.preventDefault()}
        onPointerMove={track}
        onPointerLeave={release}
        onClick={onSelect}
      >
        <span className="tile-art" ref={art}>
          <img className="tile-img" src={skin.thumbnail} alt="" draggable={false} />
        </span>
        <span className="tile-name">{skin.name}</span>
      </button>
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
          aria-label={`remove ${skin.name}`}
          title="Remove"
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
