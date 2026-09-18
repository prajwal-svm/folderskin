import type { ReactNode } from "react";
import { IconSearch } from "./icons";

export type TabCount = { id: string; label: string; count: number };

export function GalleryToolbar({
  tabs,
  active,
  onChange,
  query,
  onQuery,
  actions,
}: {
  tabs: TabCount[];
  active: string;
  onChange: (id: string) => void;
  query: string;
  onQuery: (q: string) => void;
  /** Right-aligned controls (for example the "Your photo" button). */
  actions?: ReactNode;
}) {
  return (
    <div className="toolbar" data-tauri-drag-region>
      <div className="seg" role="tablist" aria-label="skin collections">
        {tabs.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={t.id === active}
            className={t.id === active ? "seg-btn is-active" : "seg-btn"}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => onChange(t.id)}
          >
            {t.label}
            <span className="count">{t.count}</span>
          </button>
        ))}
      </div>
      <label className="search">
        <IconSearch />
        <input
          type="search"
          value={query}
          placeholder="Search skins…"
          aria-label="search skins"
          spellCheck={false}
          onChange={(e) => onQuery(e.target.value)}
        />
      </label>
      {actions && <div className="toolbar-actions">{actions}</div>}
    </div>
  );
}
