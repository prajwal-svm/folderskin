import type { ReactNode } from "react";
import type { Theme } from "../state/theme";
import { IconGlobe, IconGrid, IconSliders, IconSparkles, IconStar } from "./icons";

export type View = "skins" | "faves" | "community" | "generate";

type Item = { id: View; label: string; icon: ReactNode; badge?: string | number; soon?: boolean };

export function Sidebar({
  view,
  onView,
  favoritesCount,
  theme,
  onToggleTheme,
  onAbout,
  aboutOpen,
}: {
  view: View;
  onView: (v: View) => void;
  favoritesCount: number;
  theme: Theme;
  onToggleTheme: () => void;
  onAbout: () => void;
  aboutOpen: boolean;
}) {
  const groups: { title: string; items: Item[] }[] = [
    {
      title: "Library",
      items: [
        { id: "skins", label: "Skins", icon: <IconGrid size={18} /> },
        { id: "faves", label: "Favourites", icon: <IconStar size={17} />, badge: favoritesCount || undefined },
      ],
    },
    {
      title: "Explore",
      items: [
        { id: "community", label: "Community", icon: <IconGlobe size={18} /> },
        { id: "generate", label: "Generate with AI", icon: <IconSparkles size={18} />, soon: true },
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
            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <path d="M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5z" />
            </svg>
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
            <IconSliders size={18} />
          </span>
          <span className="nav-label">About</span>
          <span className="nav-version">v{__APP_VERSION__}</span>
        </button>
      </div>
    </nav>
  );
}
