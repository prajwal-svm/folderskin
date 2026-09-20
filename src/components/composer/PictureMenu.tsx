import { useState } from "react";
import type { Skin } from "../../lib/tauri";
import { FolderOpenIcon } from "../icons/folder-open";

/**
 * Where a picture layer's picture comes from: a file (the dialog), or one of the user's own
 * skins. Dropping or pasting a picture onto the folder does the same without this menu.
 */
export function PictureMenu({ skins, onFile, onSkin }: { skins: Skin[]; onFile: () => void; onSkin: (skin: Skin) => void }) {
  const [query, setQuery] = useState("");
  const q = query.trim().toLowerCase();
  const shown = skins.filter((s) => !q || s.name.toLowerCase().includes(q)).slice(0, 60);
  return (
    <div className="cmp-picture-menu">
      <button type="button" className="menu-item" onClick={onFile}>
        <FolderOpenIcon size={16} />
        Choose a picture
      </button>
      <p className="cmp-note">Or drop one on the folder, or paste it with ⌘V.</p>
      {skins.length > 0 && (
        <>
          <p className="cmp-picker-title">From your skins</p>
          {skins.length > 12 && (
            <input className="cmp-search" placeholder="Find a skin" value={query} onChange={(e) => setQuery(e.target.value)} aria-label="find a skin" spellCheck={false} />
          )}
          <div className="cmp-skin-grid">
            {shown.map((s) => (
              <button key={s.id} type="button" className="cmp-skin" title={s.name} aria-label={`use ${s.name}`} onClick={() => onSkin(s)}>
                <img src={s.thumbnail} alt="" draggable={false} />
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
