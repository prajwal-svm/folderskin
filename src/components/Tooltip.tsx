import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { placeTip, type TipSide } from "../lib/tip";
import { branded } from "./Brand";

/** How long the pointer rests on something before its tip shows, and how long the next one waits after a tip has just closed (none: moving along a row of buttons reads each at once). */
const DELAY_MS = 420;
const WARM_MS = 500;

type Shown = { el: HTMLElement; text: string; kbd?: string; side: TipSide };

/** Whether `el`, or something in it, is cut short: its text runs past its box. */
function cutShort(el: HTMLElement): boolean {
  const over = (e: Element) => e.scrollWidth > e.clientWidth + 1 || e.scrollHeight > e.clientHeight + 1;
  return over(el) || [...el.querySelectorAll("*")].some(over);
}

/** The element whose tip a pointer or focus event is about: the nearest one that carries one. */
function tipOwner(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof Element)) return null;
  const el = target.closest<HTMLElement>("[data-tip], [title]");
  if (!el) return null;
  // A native title would draw the system's own tooltip over ours (and never in the app's style),
  // so it becomes a tip the first time anything reaches it.
  const title = el.getAttribute("title");
  if (title !== null) {
    el.removeAttribute("title");
    if (title && !el.dataset.tip) el.dataset.tip = title;
  }
  return el.dataset.tip ? el : null;
}

/**
 * Every tooltip in the app. Anything with `data-tip` shows it after a short rest of the pointer, or
 * at once when the keyboard reaches it; `data-tip-kbd` adds a shortcut after it,
 * `data-tip-side` says where it prefers to sit (above, unless there's no room) and
 * `data-tip-overflow` shows it only when the element's text is cut short. One layer for the
 * whole window, drawn over everything, so a tip is never cut off by the scrolling list or the
 * island it sits in, and never the system's own grey box.
 */
export function TipLayer() {
  const [shown, setShown] = useState<Shown | null>(null);
  const [pos, setPos] = useState<{ x: number; y: number; side: TipSide } | null>(null);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let timer = 0;
    let current: HTMLElement | null = null;
    // A press closes the tip, and it stays closed until the pointer leaves what was pressed.
    let pressed: HTMLElement | null = null;
    let warmUntil = 0;

    const read = (el: HTMLElement): Shown => ({
      el,
      text: el.dataset.tip ?? "",
      kbd: el.dataset.tipKbd || undefined,
      side: (el.dataset.tipSide as TipSide | undefined) ?? "top",
    });
    const hide = () => {
      window.clearTimeout(timer);
      if (current) warmUntil = performance.now() + WARM_MS;
      current = null;
      setShown(null);
    };
    const showSoon = (el: HTMLElement, now: boolean) => {
      window.clearTimeout(timer);
      current = el;
      setShown(null);
      const delay = now || performance.now() < warmUntil ? 0 : DELAY_MS;
      timer = window.setTimeout(() => {
        if (current !== el || !el.isConnected || !el.dataset.tip) return;
        // A name that fits says itself; only one cut short with an ellipsis shows it whole.
        if (el.hasAttribute("data-tip-overflow") && !cutShort(el)) return;
        setShown(read(el));
      }, delay);
    };

    const onOver = (e: PointerEvent) => {
      if (e.pointerType === "touch") return;
      const el = tipOwner(e.target);
      if (pressed && el !== pressed) pressed = null;
      if (!el) {
        if (current) hide();
        return;
      }
      if (el === current || el === pressed) return;
      showSoon(el, false);
    };
    const onOut = (e: PointerEvent) => {
      // Out of the window altogether.
      if (!e.relatedTarget && current) hide();
      if (!e.relatedTarget) pressed = null;
    };
    const onDown = (e: PointerEvent) => {
      const el = tipOwner(e.target);
      if (current || el) {
        hide();
        pressed = el;
      }
    };
    const onFocus = (e: FocusEvent) => {
      const el = tipOwner(e.target);
      if (el && el.matches(":focus-visible")) showSoon(el, true);
    };
    const onBlur = (e: FocusEvent) => {
      if (current && e.target === current) hide();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && current) hide();
    };
    const onScroll = () => current && hide();

    document.addEventListener("pointerover", onOver, true);
    document.addEventListener("pointerout", onOut, true);
    document.addEventListener("pointerdown", onDown, true);
    document.addEventListener("focusin", onFocus, true);
    document.addEventListener("focusout", onBlur, true);
    document.addEventListener("keydown", onKey, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onScroll);
    window.addEventListener("blur", hide);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener("pointerover", onOver, true);
      document.removeEventListener("pointerout", onOut, true);
      document.removeEventListener("pointerdown", onDown, true);
      document.removeEventListener("focusin", onFocus, true);
      document.removeEventListener("focusout", onBlur, true);
      document.removeEventListener("keydown", onKey, true);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onScroll);
      window.removeEventListener("blur", hide);
    };
  }, []);

  // Placed once its size is known, then kept with its element: a tip whose element went away
  // (a row deleted, a view left) goes with it.
  useLayoutEffect(() => {
    if (!shown) {
      setPos(null);
      return;
    }
    let frame = 0;
    const place = () => {
      const tip = box.current;
      if (!shown.el.isConnected || !tip) {
        setShown(null);
        return;
      }
      const r = shown.el.getBoundingClientRect();
      const next = placeTip(
        { left: r.left, top: r.top, right: r.right, bottom: r.bottom },
        { w: tip.offsetWidth, h: tip.offsetHeight },
        { w: window.innerWidth, h: window.innerHeight },
        shown.side,
      );
      setPos((p) => (p && p.x === next.x && p.y === next.y && p.side === next.side ? p : next));
      frame = requestAnimationFrame(place);
    };
    place();
    return () => cancelAnimationFrame(frame);
  }, [shown]);

  if (!shown) return null;
  return createPortal(
    <div
      ref={box}
      className={`tip is-${pos?.side ?? shown.side}`}
      role="tooltip"
      style={pos ? { left: pos.x, top: pos.y } : { left: 0, top: 0, visibility: "hidden" }}
    >
      <span className="tip-text">{branded(shown.text)}</span>
      {shown.kbd && <kbd className="tip-kbd">{shown.kbd}</kbd>}
    </div>,
    document.body,
  );
}
