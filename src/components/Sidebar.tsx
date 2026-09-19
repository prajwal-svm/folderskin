import { useEffect, useRef, useState, type ReactNode } from "react";
import type { Theme } from "../state/theme";
import { EarthIcon } from "./icons/earth";
import { FolderOpenIcon } from "./icons/folder-open";
import { ImageIcon } from "./icons/image";
import { LayoutGridIcon } from "./icons/layout-grid";
import { MoonIcon } from "./icons/moon";
import { PaletteIcon } from "./icons/palette";
import { SlidersHorizontalIcon } from "./icons/sliders-horizontal";
import { SparklesIcon } from "./icons/sparkles";
import { StarIcon } from "./icons/star";

export type View = "skins" | "yours" | "faves" | "compose" | "generate" | "community";

type Item = { id: View; label: string; icon: ReactNode; badge?: string | number };

export function Sidebar({
  view,
  onView,
  skinsCount,
  favoritesCount,
  yoursCount,
  onImport,
  theme,
  onToggleTheme,
  onAboutHover,
  aboutOpen,
  updateReady,
  onSettings,
  settingsOpen,
}: {
  view: View;
  onView: (v: View) => void;
  /** Every skin in the library. Each Library item shows its count, unless it's 0. */
  skinsCount: number;
  favoritesCount: number;
  yoursCount: number;
  onImport: () => void;
  theme: Theme;
  onToggleTheme: () => void;
  /** The pointer or focus came to the version badge (true) or left it (false). */
  onAboutHover: (open: boolean) => void;
  aboutOpen: boolean;
  /** A newer FolderSkin is waiting: the badge wears a dot until it's installed. */
  updateReady: boolean;
  onSettings: () => void;
  settingsOpen: boolean;
}) {
  const groups: { title: string; items: Item[] }[] = [
    {
      title: "Library",
      items: [
        { id: "skins", label: "All skins", icon: <LayoutGridIcon size={18} />, badge: skinsCount || undefined },
        { id: "yours", label: "Yours", icon: <FolderOpenIcon size={18} />, badge: yoursCount || undefined },
        { id: "faves", label: "Favourites", icon: <StarIcon size={17} />, badge: favoritesCount || undefined },
      ],
    },
    {
      title: "Create",
      items: [
        { id: "compose", label: "Design your own", icon: <PaletteIcon size={18} /> },
        { id: "generate", label: "Generate with AI", icon: <SparklesIcon size={18} /> },
      ],
    },
    {
      title: "Explore",
      items: [{ id: "community", label: "Community", icon: <EarthIcon size={18} /> }],
    },
  ];
  const dark = theme === "dark";
  return (
    <nav className="sidebar" aria-label="sections">
      <div className="sidebar-top" data-tauri-drag-region>
        <div className="brand-lockup" aria-label={`FolderSkin version ${__APP_VERSION__}`}>
          <img className="brand-mark" src="/brand-mark.png" alt="" draggable={false} />
          <span className="brand-name">
            Folder<span className="brand-accent">Skin</span>
          </span>
          <button
            type="button"
            className={updateReady ? "brand-version has-update" : "brand-version"}
            aria-label="about FolderSkin"
            aria-expanded={aboutOpen}
            onMouseDown={(e) => e.preventDefault()}
            onMouseEnter={() => onAboutHover(true)}
            onMouseLeave={() => onAboutHover(false)}
            onFocus={() => onAboutHover(true)}
            onBlur={() => onAboutHover(false)}
            onClick={() => onAboutHover(true)}
          >
            v{__APP_VERSION__}
          </button>
        </div>
      </div>
      <button type="button" className="btn btn-primary cta" onMouseDown={(e) => e.preventDefault()} onClick={onImport}>
        <ImageIcon size={16} />
        Add your photo
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
                {item.badge !== undefined && <Badge value={item.badge} />}
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
          className={settingsOpen ? "nav-btn is-active" : "nav-btn"}
          aria-haspopup="dialog"
          onMouseDown={(e) => e.preventDefault()}
          onClick={onSettings}
        >
          <span className="nav-icon">
            <SlidersHorizontalIcon size={18} />
          </span>
          <span className="nav-label">Settings</span>
        </button>
      </div>
    </nav>
  );
}

/** A count that gives a little bump whenever it changes, so adding a favourite is felt. */
function Badge({ value }: { value: string | number }) {
  const first = useRef(true);
  const [bump, setBump] = useState(0);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    setBump((n) => n + 1);
  }, [value]);
  return (
    <span className={bump ? "nav-badge is-bumped" : "nav-badge"} key={bump}>
      {value}
    </span>
  );
}
