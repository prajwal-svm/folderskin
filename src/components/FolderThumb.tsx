import type { CSSProperties } from "react";
import type { Skin } from "../lib/tauri";
import { StarIcon } from "./icons/star";

export function FolderThumb({
  skin,
  index,
  selected,
  favorite,
  onSelect,
  onToggleFavorite,
}: {
  skin: Skin;
  index: number;
  selected: boolean;
  favorite: boolean;
  onSelect: () => void;
  onToggleFavorite: () => void;
}) {
  const cls = ["thumb-wrap", selected ? "is-selected" : "", favorite ? "is-favorite" : ""].filter(Boolean).join(" ");
  return (
    <div className={cls} style={{ "--i": index } as CSSProperties}>
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
        <StarIcon filled={favorite} />
      </button>
    </div>
  );
}
