import { useCallback, useRef, type CSSProperties, type PointerEvent } from "react";
import type { Skin } from "../lib/tauri";
import { StarIcon } from "./icons/star";

/** Degrees the folder turns at the edge of the tile, and pixels it drifts toward the pointer. */
const TILT_Y = 16;
const TILT_X = 14;
const DRIFT = 6;

const reducedMotion = () => typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * One folder in the library. It turns to face the pointer and a soft light follows the
 * pointer across the folder (and only the folder: the light is masked by the picture itself).
 * The star springs in on hover and stays while the skin is a favourite.
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
  const ref = useRef<HTMLDivElement>(null);
  const frame = useRef(0);

  const track = useCallback((e: PointerEvent<HTMLButtonElement>) => {
    const el = ref.current;
    if (!el || reducedMotion()) return;
    const r = e.currentTarget.getBoundingClientRect();
    const x = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    const y = Math.min(1, Math.max(0, (e.clientY - r.top) / r.height));
    cancelAnimationFrame(frame.current);
    frame.current = requestAnimationFrame(() => {
      el.classList.add("is-tracking");
      el.style.setProperty("--ry", `${((x - 0.5) * TILT_Y).toFixed(2)}deg`);
      el.style.setProperty("--rx", `${((0.5 - y) * TILT_X).toFixed(2)}deg`);
      el.style.setProperty("--tx", `${((x - 0.5) * DRIFT).toFixed(2)}px`);
      el.style.setProperty("--ty", `${((y - 0.5) * DRIFT).toFixed(2)}px`);
      el.style.setProperty("--gx", `${(x * 100).toFixed(1)}%`);
      el.style.setProperty("--gy", `${(y * 100).toFixed(1)}%`);
    });
  }, []);

  const release = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    cancelAnimationFrame(frame.current);
    el.classList.remove("is-tracking");
    for (const v of ["--rx", "--ry", "--tx", "--ty"]) el.style.removeProperty(v);
  }, []);

  const cls = ["tile", selected ? "is-selected" : "", favorite ? "is-favorite" : ""].filter(Boolean).join(" ");
  const mask = { maskImage: `url("${skin.thumbnail}")`, WebkitMaskImage: `url("${skin.thumbnail}")` };

  return (
    <div className={cls} style={{ "--i": Math.min(index, 24) } as CSSProperties} ref={ref}>
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
        <span className="tile-art">
          <img className="tile-img" src={skin.thumbnail} alt="" draggable={false} />
          <span className="tile-glare" style={mask} aria-hidden="true" />
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
