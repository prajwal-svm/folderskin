import { useEffect, useLayoutEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { branded } from "./Brand";
import { t } from "../i18n";

/**
 * The dialogs open now, oldest first. Only the newest answers Escape and keeps the focus, so a
 * "Are you sure?" over another dialog closes on its own instead of taking the one below with it.
 */
const open: symbol[] = [];

/** The app behind a dialog can't be clicked, tabbed into or read out while one is open. */
function setAppInert(inert: boolean) {
  const root = document.getElementById("root");
  if (!root) return;
  if (inert) root.setAttribute("inert", "");
  else root.removeAttribute("inert");
}

const FOCUSABLE = "a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])";

/**
 * A dialog over a blurred backdrop. Escape or a click outside closes it; focus moves into it
 * on open, Tab stays inside it, and focus goes back to whatever had it on close.
 */
export function Modal({
  title,
  sub,
  onClose,
  children,
  footer,
  wide,
  narrow,
  className,
  closable = true,
  bare,
}: {
  title: string;
  sub?: ReactNode;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  /** For short questions, like "are you sure?". */
  narrow?: boolean;
  /** A width or treatment of its own, for a dialog that needs more room than the three sizes. */
  className?: string;
  /** False while something runs that mustn't be left: no close button, and Escape and a click outside do nothing. */
  closable?: boolean;
  /** No heading or padded body: the dialog lays out its own inside, with the close button floating top right. `title` still names it. */
  bare?: boolean;
}) {
  const panel = useRef<HTMLDivElement>(null);
  // The latest onClose, so a parent that passes a new function each render doesn't re-run the
  // focus handling below and pull the focus back to the first field.
  const close = useRef(onClose);
  const canClose = useRef(closable);
  useLayoutEffect(() => {
    close.current = onClose;
    canClose.current = closable;
  });

  useEffect(() => {
    const me = Symbol("modal");
    open.push(me);
    setAppInert(true);
    const before = document.activeElement as HTMLElement | null;
    // The field the dialog asks for, or else whatever comes first: a form whose first control is
    // a filter shouldn't open with the filter focused.
    const first =
      panel.current?.querySelector<HTMLElement>("[data-modal-focus]") ??
      panel.current?.querySelector<HTMLElement>("input, textarea, select, button:not(.modal-close)");
    first?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (open[open.length - 1] !== me) return;
      if (e.key === "Escape") {
        // A list opened from the dialog, like a Select's, closes first and the dialog stays: the
        // list hears Escape on the document, after this. Opened after the dialog, it comes after
        // it in the page.
        const pops = document.querySelectorAll(".cmp-pop");
        if ([...pops].some((p) => panel.current && panel.current.compareDocumentPosition(p) & Node.DOCUMENT_POSITION_FOLLOWING)) return;
        // Nothing else hears it: not a dialog underneath, not the composer's shortcuts.
        e.stopImmediatePropagation();
        // A search field with words in it empties itself first, which is what the browser does
        // with Escape there.
        const t = e.target;
        if (t instanceof HTMLInputElement && t.type === "search" && t.value) return;
        e.preventDefault();
        if (canClose.current) close.current();
      } else if (e.key === "Tab" && panel.current) {
        // Only what Tab would stop at: one control of a group whose arrow keys move within it
        // (the rest have a tabindex of -1), and the one that has the focus, wherever it is.
        const items = [...panel.current.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
          (el) => el === document.activeElement || (el.tabIndex >= 0 && el.offsetParent !== null),
        );
        if (items.length === 0) return;
        const at = items.indexOf(document.activeElement as HTMLElement);
        const next = e.shiftKey ? (at <= 0 ? items.length - 1 : at - 1) : at === -1 || at === items.length - 1 ? 0 : at + 1;
        e.preventDefault();
        items[next].focus();
      }
    };
    // Capture, so a dialog hears Escape before anything in the app it covers.
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      open.splice(open.indexOf(me), 1);
      if (open.length === 0) setAppInert(false);
      before?.focus?.();
    };
  }, []);

  return createPortal(
    <div
      className="modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && closable) onClose();
      }}
    >
      <div className={[ "modal", wide && "modal-wide", narrow && "modal-narrow", className ].filter(Boolean).join(" ")} role="dialog" aria-modal="true" aria-label={title} ref={panel}>
        {bare ? (
          <>
            {children}
            {closable && closeButton(onClose, "modal-close is-floating")}
          </>
        ) : (
          <>
            <header className="modal-head">
              <div style={{ flex: 1, minWidth: 0 }}>
                <h2 className="modal-title">{branded(title)}</h2>
                {sub && <p className="modal-sub">{branded(sub)}</p>}
              </div>
              {closable && closeButton(onClose, "modal-close")}
            </header>
            <div className="modal-body">{children}</div>
            {footer && <footer className="modal-foot">{footer}</footer>}
          </>
        )}
      </div>
    </div>,
    document.body,
  );
}

function closeButton(onClose: () => void, className: string) {
  return (
    <button type="button" className={`icon-btn ${className}`} aria-label={t("common.close")} onClick={onClose}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" aria-hidden="true">
        <path d="M18 6 6 18M6 6l12 12" />
      </svg>
    </button>
  );
}
