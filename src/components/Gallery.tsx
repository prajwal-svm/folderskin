import type { Skin } from "../lib/tauri";
import { FolderThumb } from "./FolderThumb";

export function Gallery({
  skins,
  selectedId,
  favorites,
  emptyMessage,
  onSelect,
  onToggleFavorite,
}: {
  skins: Skin[];
  selectedId: string | null;
  favorites: string[];
  emptyMessage: string | null;
  onSelect: (id: string) => void;
  onToggleFavorite: (id: string) => void;
}) {
  if (emptyMessage) return <p className="gallery-empty">{emptyMessage}</p>;
  return (
    <div className="gallery" role="list">
      {skins.map((skin) => (
        <div role="listitem" key={skin.id}>
          <FolderThumb
            skin={skin}
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
