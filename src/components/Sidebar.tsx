import type { ReactNode } from "react";
import type { Theme } from "../state/theme";
import { EarthIcon } from "./icons/earth";
import { ImageIcon } from "./icons/image";
import { LayoutGridIcon } from "./icons/layout-grid";
import { MoonIcon } from "./icons/moon";
import { SlidersHorizontalIcon } from "./icons/sliders-horizontal";
import { SparklesIcon } from "./icons/sparkles";
import { StarIcon } from "./icons/star";

export type View = "skins" | "faves" | "community" | "generate";

type Item = { id: View; label: string; icon: ReactNode; badge?: string | number; soon?: boolean };

export function Sidebar({
  view,
  onView,
  favoritesCount,
  onImport,
  theme,
  onToggleTheme,
  onAbout,
  aboutOpen,
}: {
  view: View;
  onView: (v: View) => void;
  favoritesCount: number;
  onImport: () => void;
  theme: Theme;
  onToggleTheme: () => void;
  onAbout: () => void;
  aboutOpen: boolean;
}) {
  const groups: { title: string; items: Item[] }[] = [
    {
      title: "Library",
      items: [
        { id: "skins", label: "Skins", icon: <LayoutGridIcon size={18} /> },
        { id: "faves", label: "Favourites", icon: <StarIcon size={17} />, badge: favoritesCount || undefined },
      ],
    },
    {
      title: "Explore",
      items: [
        { id: "community", label: "Community", icon: <EarthIcon size={18} /> },
        { id: "generate", label: "Generate with AI", icon: <SparklesIcon size={18} /> },
      ],
    },
  ];
  const dark = theme === "dark";
  return (
    <nav className="sidebar" aria-label="sections">
      <div className="sidebar-top" data-tauri-drag-region>
        <div className="brand-lockup" aria-label="FolderSkin">
          <img className="brand-icon" src="/app-icon.png" alt="" draggable={false} />
          <span className="brand-name">FolderSkin</span>
        </div>
      </div>
      <button type="button" className="btn btn-primary cta" onMouseDown={(e) => e.preventDefault()} onClick={onImport}>
        <span className="btn-badge">
          <ImageIcon size={14} />
        </span>
        Your photo
      </button>
      <div className="sidebar-scroll">
        {groups.map((g) => (
          <div className="nav-group" key={g.title}>
            <p className="nav-title">{g.title}</p>
            {g.items.map((item) => (
              <button
                key={item.id}
                type="button"
                className={item.id === view ? "nav-btn is-active" : "nav-btn"}
                aria-current={item.id === view ? "page" : undefined}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => onView(item.id)}
              >
                <span className="nav-icon">{item.icon}</span>
                <span className="nav-label">{item.label}</span>
                {item.badge !== undefined && <span className="nav-badge">{item.badge}</span>}
                {item.soon && <span className="nav-soon">soon</span>}
              </button>
            ))}
          </div>
        ))}
      </div>
      <div className="sidebar-bottom">
        <button
          type="button"
          className="nav-btn nav-switch"
          role="switch"
          aria-checked={dark}
          aria-label="dark mode"
          onMouseDown={(e) => e.preventDefault()}
          onClick={onToggleTheme}
        >
          <span className="nav-icon">
            <MoonIcon />
          </span>
          <span className="nav-label">Dark mode</span>
          <span className={dark ? "switch is-on" : "switch"} aria-hidden="true">
            <span className="knob" />
          </span>
        </button>
        <button
          type="button"
          className={aboutOpen ? "nav-btn is-active" : "nav-btn"}
          aria-label="about FolderSkin"
          aria-expanded={aboutOpen}
          onMouseDown={(e) => e.preventDefault()}
          onClick={onAbout}
        >
          <span className="nav-icon">
            <SlidersHorizontalIcon size={18} />
          </span>
          <span className="nav-label">About</span>
          <span className="nav-version">v{__APP_VERSION__}</span>
        </button>
      </div>
    </nav>
  );
}
