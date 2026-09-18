import type { Skin } from "../lib/tauri";
import { FolderThumb } from "./FolderThumb";

export function Gallery({
  skins,
  selectedId,
  favorites,
  emptyMessage,
  animationKey,
  onSelect,
  onToggleFavorite,
}: {
  skins: Skin[];
  selectedId: string | null;
  favorites: string[];
  emptyMessage: string | null;
  /** Changes when the visible set changes so the grid re-runs its entrance animation. */
  animationKey: string;
  onSelect: (id: string) => void;
  onToggleFavorite: (id: string) => void;
}) {
  if (emptyMessage) return <p className="gallery-empty">{emptyMessage}</p>;
  return (
    <div className="gallery" role="list" key={animationKey}>
      {skins.map((skin, i) => (
        <div role="listitem" key={skin.id}>
          <FolderThumb
            skin={skin}
            index={i}
            selected={skin.id === selectedId}
            favorite={favorites.includes(skin.id)}
            onSelect={() => onSelect(skin.id)}
            onToggleFavorite={() => onToggleFavorite(skin.id)}
          />
        </div>
      ))}
    </div>
  );
}
