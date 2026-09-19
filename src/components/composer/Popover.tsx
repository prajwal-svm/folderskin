import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

const MARGIN = 10;

/**
 * A panel floating beside the control that opened it: below it when there is room, above it
 * otherwise, kept inside the window. Escape or a click anywhere else closes it; the control that
 * opened it doesn't count as elsewhere, so clicking it again can close it.
 */
export function Popover({
  anchor,
  onClose,
  children,
  width,
  align = "start",
  className,
  label,
}: {
  anchor: HTMLElement;
  onClose: () => void;
  children: ReactNode;
  width: number;
  /** Which edge of the control the panel lines up with. */
  align?: "start" | "end" | "center";
  className?: string;
  label: string;
}) {
  const panel = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number; maxHeight: number; above: boolean } | null>(null);
  const close = useRef(onClose);
  useLayoutEffect(() => {
    close.current = onClose;
  });

  useLayoutEffect(() => {
    const place = () => {
      const a = anchor.getBoundingClientRect();
      const h = panel.current?.scrollHeight ?? 0;
      const x = align === "end" ? a.right - width : align === "center" ? a.left + a.width / 2 - width / 2 : a.left;
      const left = Math.min(Math.max(MARGIN, x), window.innerWidth - width - MARGIN);
      const below = window.innerHeight - a.bottom - MARGIN - 6;
      const aboveRoom = a.top - MARGIN - 6;
      const above = h > below && aboveRoom > below;
      const maxHeight = Math.max(160, above ? aboveRoom : below);
      const top = above ? Math.max(MARGIN, a.top - 6 - Math.min(h, maxHeight)) : a.bottom + 6;
      setPos({ left, top, maxHeight, above });
    };
    place();
    const grow = new ResizeObserver(place);
    if (panel.current) grow.observe(panel.current);
    window.addEventListener("resize", place);
    return () => {
      grow.disconnect();
      window.removeEventListener("resize", place);
    };
  }, [anchor, width, align]);

  useEffect(() => {
    panel.current?.focus({ preventScroll: true });
    const down = (e: MouseEvent) => {
      const t = e.target as Node;
      if (panel.current?.contains(t) || anchor.contains(t)) return;
      // A popover opened from inside this one (a colour picker) is part of it.
      if (t instanceof Element && t.closest(".cmp-pop")) return;
      close.current();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      close.current();
      anchor.focus({ preventScroll: true });
    };
    document.addEventListener("mousedown", down);
    document.addEventListener("keydown", key, true);
    return () => {
      document.removeEventListener("mousedown", down);
      document.removeEventListener("keydown", key, true);
    };
  }, [anchor]);

  return createPortal(
    <div
      ref={panel}
      className={`cmp-pop${pos?.above ? " is-above" : ""}${className ? ` ${className}` : ""}`}
      role="dialog"
      aria-label={label}
      tabIndex={-1}
      style={pos ? { left: pos.left, top: pos.top, width, maxHeight: pos.maxHeight } : { visibility: "hidden", width }}
    >
      {children}
    </div>,
    document.body,
  );
}
