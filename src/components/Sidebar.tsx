import { useEffect, useRef, useState, type ReactNode } from "react";
import type { Theme } from "../state/theme";
import { keys } from "../lib/platform";
import { useT } from "../i18n";
import { formatNumber } from "../i18n/format";
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
import { LanguageMenu } from "./LanguageMenu";

export type View = "skins" | "yours" | "faves" | "compose" | "generate" | "community";

type Item = { id: View; label: string; icon: ReactNode; badge?: number };

/** The shortcut that folds the sidebar and opens it again (App.tsx), as its tooltip says it. */
const FOLD_KEYS = keys("\\");

/**
 * The app's sections, the photo button, dark mode, the language and Settings. Open, it sits
 * straight on the window's glass with names beside its icons. Folded to a rail, it becomes an
 * island of its own: the logo with no name or version, and the icons alone, each still a button,
 * named in a tooltip.
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
  const t = useT();
  const groups: { id: string; title: string; items: Item[] }[] = [
    {
      id: "library",
      title: t("sidebar.groups.library"),
      items: [
        { id: "skins", label: t("sidebar.items.skins"), icon: <LayoutGridIcon size={18} />, badge: skinsCount || undefined },
        { id: "yours", label: t("sidebar.items.yours"), icon: <FolderOpenIcon size={18} />, badge: yoursCount || undefined },
        { id: "faves", label: t("sidebar.items.faves"), icon: <StarIcon size={17} />, badge: favoritesCount || undefined },
      ],
    },
    {
      id: "create",
      title: t("sidebar.groups.create"),
      items: [
        { id: "compose", label: t("sidebar.items.compose"), icon: <PaletteIcon size={18} /> },
        { id: "generate", label: t("sidebar.items.generate"), icon: <SparklesIcon size={18} /> },
      ],
    },
    {
      id: "explore",
      title: t("sidebar.groups.explore"),
      items: [{ id: "community", label: t("sidebar.items.community"), icon: <EarthIcon size={18} /> }],
    },
  ];
  const dark = theme === "dark";
  const addPhoto = t("sidebar.addPhoto");
  // Folded, the buttons have no names beside them, so each says what it is in a tooltip.
  const tip = (text: string) => (rail ? { "data-tip": text, "data-tip-side": "right" } : {});
  return (
    <nav className={rail ? "sidebar is-rail" : "sidebar"} aria-label={t("sidebar.sections")}>
      <div className="sidebar-top" data-tauri-drag-region>
        <div className="brand-lockup" aria-label={t("sidebar.version", { version: __APP_VERSION__ })}>
          <img className="brand-mark" src="/brand-mark.png" alt="" draggable={false} />
          {!rail && (
            <>
              <span className="brand-name">
                Folder<span className="brand-accent">Skin</span>
              </span>
              <button
                type="button"
                className={updateReady ? "brand-version has-update" : "brand-version"}
                aria-label={t("sidebar.about")}
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
        aria-label={rail ? addPhoto : undefined}
        onMouseDown={(e) => e.preventDefault()}
        onClick={onImport}
        {...tip(addPhoto)}
      >
        <ImageIcon size={16} />
        {!rail && <span className="cta-label">{addPhoto}</span>}
      </button>
      <div className="sidebar-scroll">
        {groups.map((g) => (
          <div className="nav-group" key={g.id} role="group" aria-label={g.title}>
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
                {...tip(item.badge !== undefined ? t("sidebar.withCount", { label: item.label, count: formatNumber(item.badge) }) : item.label)}
              >
                <span className="nav-icon">{item.icon}</span>
                {!rail && <span className="nav-label">{item.label}</span>}
                {!rail && item.badge !== undefined && <Badge value={formatNumber(item.badge)} />}
              </button>
            ))}
          </div>
        ))}
      </div>
      <div className="sidebar-bottom">
        <button
          type="button"
          className="nav-btn"
          aria-label={rail ? t("sidebar.expandLabel") : t("sidebar.collapseLabel")}
          aria-expanded={!rail}
          data-tip={rail ? t("sidebar.expandTip") : t("sidebar.collapseTip")}
          data-tip-kbd={FOLD_KEYS}
          data-tip-side="right"
          onMouseDown={(e) => e.preventDefault()}
          onClick={onToggleRail}
        >
          <span className="nav-icon">{rail ? <PanelLeftOpenIcon size={18} /> : <PanelLeftCloseIcon size={18} />}</span>
          {!rail && <span className="nav-label">{t("sidebar.collapse")}</span>}
        </button>
        <button
          type="button"
          className="nav-btn nav-switch"
          role="switch"
          aria-checked={dark}
          aria-label={t("sidebar.darkModeLabel")}
          onMouseDown={(e) => e.preventDefault()}
          onClick={onToggleTheme}
          {...tip(dark ? t("sidebar.darkOn") : t("sidebar.darkOff"))}
        >
          <span className="nav-icon">{rail && !dark ? <SunIcon size={18} /> : <MoonIcon />}</span>
          {!rail && (
            <>
              <span className="nav-label">{t("sidebar.darkMode")}</span>
              <span className={dark ? "switch is-on" : "switch"} aria-hidden="true">
                <span className="knob" />
              </span>
            </>
          )}
        </button>
        <LanguageMenu rail={rail} />
        <button
          type="button"
          className={settingsOpen ? "nav-btn is-active" : "nav-btn"}
          aria-haspopup="dialog"
          aria-label={rail ? t("sidebar.settings") : undefined}
          onMouseDown={(e) => e.preventDefault()}
          onClick={onSettings}
          {...tip(t("sidebar.settings"))}
        >
          <span className="nav-icon">
            <SlidersHorizontalIcon size={18} />
          </span>
          {!rail && <span className="nav-label">{t("sidebar.settings")}</span>}
        </button>
      </div>
    </nav>
  );
}

/** A count that gives a little bump whenever it changes, so adding a favourite is felt. */
function Badge({ value }: { value: string }) {
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
