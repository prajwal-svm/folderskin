import { useEffect, useId, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { LOCALE_NAMES, LOCALES, useLocale, useT, type Locale } from "../i18n";
import { chooseLanguage } from "../state/language";
import { TickIcon } from "./icons/composer";
import { LanguagesIcon } from "./icons/languages";

/** Room between the row and its menu, and the least room left at the window's edge. */
const GAP = 6;
const EDGE = 8;
const WIDTH = 188;

type Place = { left: number; bottom: number; width: number };

/**
 * The sidebar's language row, under Dark mode: the language on show, in its own words, opening a
 * short menu of every language the app speaks, each in its own words and the one on show ticked.
 * Folded to a rail it's the icon alone, named in its tooltip, and the menu opens beside it. Arrow
 * keys, Home and End move through the menu, Enter or Space chooses, and Escape or Tab closes it
 * with the focus back on the row.
 */
export function LanguageMenu({ rail }: { rail: boolean }) {
  const t = useT();
  const locale = useLocale();
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [place, setPlace] = useState<Place | null>(null);
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const id = useId();
  const name = LOCALE_NAMES[locale];
  const label = t("sidebar.language.label", { language: name });

  const show = () => {
    setActive(Math.max(0, LOCALES.indexOf(locale)));
    setOpen(true);
  };
  const close = (refocus: boolean) => {
    setOpen(false);
    setPlace(null);
    if (refocus) button.current?.focus({ preventScroll: true });
  };
  const choose = (next: Locale) => {
    close(true);
    // A language that won't load leaves the app as it was.
    if (next !== locale) chooseLanguage(next).catch(() => {});
  };

  // Above the row in the open sidebar, which ends just under it; beside the icon on the rail.
  useLayoutEffect(() => {
    if (!open) return;
    const put = () => {
      const r = button.current?.getBoundingClientRect();
      if (!r) return;
      const width = rail ? WIDTH : Math.max(r.width, WIDTH);
      const left = Math.min(rail ? r.right + GAP + 4 : r.left, window.innerWidth - width - EDGE);
      const bottom = rail ? window.innerHeight - r.bottom : window.innerHeight - r.top + GAP;
      setPlace({ left: Math.max(EDGE, left), bottom: Math.max(EDGE, bottom), width });
    };
    put();
    window.addEventListener("resize", put);
    return () => window.removeEventListener("resize", put);
  }, [open, rail]);

  // The language with the keyboard has the focus, once the menu is where it goes.
  useEffect(() => {
    if (!open || !place) return;
    menu.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.focus({ preventScroll: true });
  }, [open, place, active]);

  // A press anywhere else closes it; the row itself toggles it.
  useEffect(() => {
    if (!open) return;
    const down = (e: MouseEvent) => {
      const target = e.target as Node;
      if (menu.current?.contains(target) || button.current?.contains(target)) return;
      close(false);
    };
    document.addEventListener("mousedown", down);
    return () => document.removeEventListener("mousedown", down);
  }, [open]);

  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const last = LOCALES.length - 1;
    const to = { ArrowDown: active === last ? 0 : active + 1, ArrowUp: active === 0 ? last : active - 1, Home: 0, End: last }[e.key];
    if (to !== undefined) {
      e.preventDefault();
      setActive(to);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      choose(LOCALES[active]);
    } else if (e.key === "Escape") {
      // Handled here: the library's own Escape (putting a skin down) sees it was.
      e.preventDefault();
      e.stopPropagation();
      close(true);
    } else if (e.key === "Tab") {
      e.preventDefault();
      close(true);
    }
  };

  return (
    <>
      <button
        ref={button}
        type="button"
        className={open ? "nav-btn nav-lang is-open" : "nav-btn nav-lang"}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        aria-label={label}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => (open ? close(true) : show())}
        onKeyDown={(e) => {
          if (open || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
          e.preventDefault();
          show();
        }}
        {...(rail ? { "data-tip": label, "data-tip-side": "right" } : {})}
      >
        <span className="nav-icon">
          <LanguagesIcon size={18} />
        </span>
        {!rail && (
          <span className="nav-label" lang={locale}>
            {name}
          </span>
        )}
      </button>
      {open &&
        createPortal(
          <div
            ref={menu}
            id={id}
            role="menu"
            aria-label={t("sidebar.language.menu")}
            className={rail ? "lang-pop is-beside" : "lang-pop"}
            onKeyDown={onKey}
            style={place ? { left: place.left, bottom: place.bottom, width: place.width } : { visibility: "hidden" }}
          >
            {LOCALES.map((l, i) => (
              <div
                key={l}
                role="menuitemradio"
                aria-checked={l === locale}
                lang={l}
                data-index={i}
                tabIndex={i === active ? 0 : -1}
                className={i === active ? "lang-option is-active" : "lang-option"}
                onMouseEnter={() => setActive(i)}
                onClick={() => choose(l)}
              >
                <span className="lang-option-name">{LOCALE_NAMES[l]}</span>
                {l === locale && <TickIcon size={15} />}
              </div>
            ))}
          </div>,
          document.body,
        )}
    </>
  );
}
