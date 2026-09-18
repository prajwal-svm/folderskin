import type { Skin } from "../lib/tauri";
import { IconStar } from "./icons";

export function Handles() {
  return (
    <>
      <span className="handle tl" />
      <span className="handle tr" />
      <span className="handle bl" />
      <span className="handle br" />
    </>
  );
}

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
  const cls = ["thumb-wrap", selected ? "is-selected sel" : "", favorite ? "is-favorite" : ""].filter(Boolean).join(" ");
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
      <span className="thumb-name">{skin.name}</span>
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
        <IconStar filled={favorite} />
      </button>
      {selected && <Handles />}
    </div>
  );
}
