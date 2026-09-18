import type { ReactNode } from "react";
import type { Skin } from "../lib/tauri";
import { FolderThumb } from "./FolderThumb";

export type Empty = { icon: ReactNode; title: string; text: string; action?: ReactNode };

export function Gallery({
  skins,
  selectedId,
  favorites,
  empty,
  animationKey,
  onAdd,
  onSelect,
  onToggleFavorite,
  onRemove,
  onRename,
}: {
  skins: Skin[];
  selectedId: string | null;
  favorites: string[];
  /** Shown instead of the grid when there is nothing to list. */
  empty: Empty | null;
  /** Changes when the visible set changes so the grid re-runs its entrance animation. */
  animationKey: string;
  /** Adds a first tile that opens the picture picker (the Yours tab). */
  onAdd?: () => void;
  onSelect: (id: string) => void;
  onToggleFavorite: (id: string) => void;
  /** Asks to delete one of the user's skins. */
  onRemove: (skin: Skin) => void;
  onRename: (skin: Skin, name: string) => void;
}) {
  if (empty && !onAdd) {
    return (
      <div className="empty" key={animationKey}>
        <span className="empty-glyph">{empty.icon}</span>
        <p className="empty-title">{empty.title}</p>
        <p className="empty-text">{empty.text}</p>
        {empty.action}
      </div>
    );
  }
  return (
    <div className="gallery" role="list" key={animationKey}>
      {onAdd && (
        <div role="listitem" className="tile tile-add">
          <button type="button" className="tile-hit" onMouseDown={(e) => e.preventDefault()} onClick={onAdd}>
            <span className="tile-art">
              <span className="tile-add-plus" aria-hidden="true">
                +
              </span>
              <span className="tile-add-label">Add a picture</span>
            </span>
            <span className="tile-name">
              <span className="tile-name-text">PNG, JPEG, WebP, HEIC</span>
            </span>
          </button>
        </div>
      )}
      {skins.map((skin, i) => (
        <div role="listitem" key={skin.id}>
          <FolderThumb
            skin={skin}
            index={onAdd ? i + 1 : i}
            selected={skin.id === selectedId}
            favorite={favorites.includes(skin.id)}
            onSelect={() => onSelect(skin.id)}
            onToggleFavorite={() => onToggleFavorite(skin.id)}
            onRemove={skin.custom ? () => onRemove(skin) : undefined}
            onRename={skin.custom ? (name) => onRename(skin, name) : undefined}
          />
        </div>
      ))}
    </div>
  );
}
