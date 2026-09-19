import { useEffect, useLayoutEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";

/**
 * A dialog over a blurred backdrop. Escape or a click outside closes it; focus moves into it
 * on open and back to whatever had it on close.
 */
export function Modal({
  title,
  sub,
  onClose,
  children,
  footer,
  wide,
  narrow,
  closable = true,
}: {
  title: string;
  sub?: ReactNode;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  /** For short questions, like "are you sure?". */
  narrow?: boolean;
  /** False while something runs that mustn't be left: no close button, and Escape and a click outside do nothing. */
  closable?: boolean;
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
    const before = document.activeElement as HTMLElement | null;
    const first = panel.current?.querySelector<HTMLElement>("input, textarea, select, button:not(.modal-close)");
    first?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        if (canClose.current) close.current();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
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
      <div className={wide ? "modal modal-wide" : narrow ? "modal modal-narrow" : "modal"} role="dialog" aria-modal="true" aria-label={title} ref={panel}>
        <header className="modal-head">
          <div style={{ flex: 1, minWidth: 0 }}>
            <h2 className="modal-title">{title}</h2>
            {sub && <p className="modal-sub">{sub}</p>}
          </div>
          {closable && (
            <button type="button" className="icon-btn modal-close" aria-label="close" onClick={onClose}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" aria-hidden="true">
                <path d="M18 6 6 18M6 6l12 12" />
              </svg>
            </button>
          )}
        </header>
        <div className="modal-body">{children}</div>
        {footer && <footer className="modal-foot">{footer}</footer>}
      </div>
    </div>,
    document.body,
  );
}
