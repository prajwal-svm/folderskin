import type { Skin } from "../lib/tauri";

export function FolderThumb({
  skin,
  selected,
  favorite,
  onSelect,
  onToggleFavorite,
}: {
  skin: Skin;
  selected: boolean;
  favorite: boolean;
  onSelect: () => void;
  onToggleFavorite: () => void;
}) {
  const cls = ["thumb-wrap", selected ? "is-selected" : "", favorite ? "is-favorite" : ""].filter(Boolean).join(" ");
  return (
    <div className={cls}>
      <button
        type="button"
        className="thumb"
        aria-pressed={selected}
        aria-label={skin.custom ? `${skin.name} (your picture)` : skin.name}
        title={skin.name}
        onMouseDown={(e) => e.preventDefault()}
        onClick={onSelect}
      >
        <img src={skin.thumbnail} alt="" draggable={false} />
      </button>
      <button
        type="button"
        className="star"
        aria-label={favorite ? `remove ${skin.name} from favourites` : `add ${skin.name} to favourites`}
        aria-pressed={favorite}
        onMouseDown={(e) => e.preventDefault()}
        onClick={(e) => {
          e.stopPropagation();
          onToggleFavorite();
        }}
      >
        <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true">
          <path
            d="M12 2.8l2.8 5.9 6.4.8-4.7 4.4 1.2 6.4L12 17.2l-5.7 3.1 1.2-6.4L2.8 9.5l6.4-.8z"
            fill={favorite ? "currentColor" : "none"}
            stroke="currentColor"
            strokeWidth="2"
            strokeLinejoin="round"
          />
        </svg>
      </button>
    </div>
  );
}
