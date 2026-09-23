import { useEffect, useRef, useState, type ReactNode } from "react";
import type { Theme } from "../state/theme";
import { keys } from "../lib/platform";
import { EarthIcon } from "./icons/earth";
import { FolderOpenIcon } from "./icons/folder-open";
import { ImageIcon } from "./icons/image";
import { LayoutGridIcon } from "./icons/layout-grid";
import { MoonIcon } from "./icons/moon";
import { PaletteIcon } from "./icons/palette";
import { PanelLeftCloseIcon, PanelLeftOpenIcon } from "./icons/panel-left";
import { SlidersHorizontalIcon } from "./icons/sliders-horizontal";
import { SparklesIcon } from "./icons/sparkles";
import { StarIcon } from "./icons/star";
import { SunIcon } from "./icons/sun";

export type View = "skins" | "yours" | "faves" | "compose" | "generate" | "community";

type Item = { id: View; label: string; icon: ReactNode; badge?: string | number };

/** The shortcut that folds the sidebar and opens it again (App.tsx), as its tooltip says it. */
const FOLD_KEYS = keys("\\");

/**
 * The app's sections, the photo button, dark mode and Settings. Open, it sits straight on the
 * window's glass with names beside its icons. Folded to a rail, it becomes an island of its own:
 * the logo with no name or version, and the icons alone, each still a button, named in a tooltip.
 */
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
  rail,
  onToggleRail,
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
  /** Folded to a rail of icons. */
  rail: boolean;
  onToggleRail: () => void;
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
  // Folded, the buttons have no names beside them, so each says what it is in a tooltip.
  const tip = (text: string) => (rail ? { "data-tip": text, "data-tip-side": "right" } : {});
  return (
    <nav className={rail ? "sidebar is-rail" : "sidebar"} aria-label="sections">
      <div className="sidebar-top" data-tauri-drag-region>
        <div className="brand-lockup" aria-label={`FolderSkin version ${__APP_VERSION__}`}>
          <img className="brand-mark" src="/brand-mark.png" alt="" draggable={false} />
          {!rail && (
            <>
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
            </>
          )}
        </div>
      </div>
      <button
        type="button"
        className="btn btn-primary cta"
        aria-label={rail ? "Add your photo" : undefined}
        onMouseDown={(e) => e.preventDefault()}
        onClick={onImport}
        {...tip("Add your photo")}
      >
        <ImageIcon size={16} />
        {!rail && "Add your photo"}
      </button>
      <div className="sidebar-scroll">
        {groups.map((g) => (
          <div className="nav-group" key={g.title} role="group" aria-label={g.title}>
            {!rail && <p className="nav-title">{g.title}</p>}
            {g.items.map((item) => (
              <button
                key={item.id}
                type="button"
                className={item.id === view ? "nav-btn is-active" : "nav-btn"}
                aria-current={item.id === view ? "page" : undefined}
                aria-label={rail ? item.label : undefined}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => onView(item.id)}
                {...tip(item.badge !== undefined ? `${item.label} · ${item.badge}` : item.label)}
              >
                <span className="nav-icon">{item.icon}</span>
                {!rail && <span className="nav-label">{item.label}</span>}
                {!rail && item.badge !== undefined && <Badge value={item.badge} />}
              </button>
            ))}
          </div>
        ))}
      </div>
      <div className="sidebar-bottom">
        <button
          type="button"
          className="nav-btn"
          aria-label={rail ? "expand the sidebar" : "collapse the sidebar"}
          aria-expanded={!rail}
          data-tip={rail ? "Expand the sidebar" : "Collapse the sidebar"}
          data-tip-kbd={FOLD_KEYS}
          data-tip-side="right"
          onMouseDown={(e) => e.preventDefault()}
          onClick={onToggleRail}
        >
          <span className="nav-icon">{rail ? <PanelLeftOpenIcon size={18} /> : <PanelLeftCloseIcon size={18} />}</span>
          {!rail && <span className="nav-label">Collapse sidebar</span>}
        </button>
        <button
          type="button"
          className="nav-btn nav-switch"
          role="switch"
          aria-checked={dark}
          aria-label="dark mode"
          onMouseDown={(e) => e.preventDefault()}
          onClick={onToggleTheme}
          {...tip(dark ? "Dark mode is on" : "Dark mode is off")}
        >
          <span className="nav-icon">{rail && !dark ? <SunIcon size={18} /> : <MoonIcon />}</span>
          {!rail && (
            <>
              <span className="nav-label">Dark mode</span>
              <span className={dark ? "switch is-on" : "switch"} aria-hidden="true">
                <span className="knob" />
              </span>
            </>
          )}
        </button>
        <button
          type="button"
          className={settingsOpen ? "nav-btn is-active" : "nav-btn"}
          aria-haspopup="dialog"
          aria-label={rail ? "Settings" : undefined}
          onMouseDown={(e) => e.preventDefault()}
          onClick={onSettings}
          {...tip("Settings")}
        >
          <span className="nav-icon">
            <SlidersHorizontalIcon size={18} />
          </span>
          {!rail && <span className="nav-label">Settings</span>}
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
