import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { ChevronDownIcon, TickIcon } from "./icons/composer";
import { Popover } from "./composer/Popover";

export type SelectOption<T> = { value: T; label: string; /** Drawn instead of the label, in the list and the button. */ render?: ReactNode };

/**
 * A choice from a short list, in the app's own dropdown rather than the system's: the button shows
 * what's chosen, and the list opens under it with a tick beside it. Arrow keys, Home and End move
 * through the list, a letter jumps to the next option starting with it, Enter or Space chooses and
 * Escape leaves it as it was.
 */
export function Select<T extends string | number>({
  value,
  options,
  onChange,
  label,
  className,
  placeholder,
}: {
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  /** What's being chosen, for screen readers and the list's name. */
  label: string;
  className?: string;
  /** Shown, with nothing ticked, while `value` is none of the options: for a choice the app
   *  leaves to them rather than making it. */
  placeholder?: string;
}) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const found = options.findIndex((o) => o.value === value);
  const chosen = Math.max(0, found);
  const [active, setActive] = useState(chosen);
  const list = useRef<HTMLDivElement>(null);
  const id = useId();
  const current = found === -1 && placeholder !== undefined ? null : options[chosen];

  // The option with the keyboard has the focus. A frame later: the popover is placed (and can take
  // focus) only once it has been measured, and it focuses its own panel as it opens.
  useEffect(() => {
    if (!anchor) return;
    const f = requestAnimationFrame(() => list.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.focus());
    return () => cancelAnimationFrame(f);
  }, [anchor, active]);

  const open = (el: HTMLElement) => {
    setActive(chosen);
    setAnchor(el);
  };
  const close = () => {
    const a = anchor;
    setAnchor(null);
    a?.focus({ preventScroll: true });
  };
  const choose = (i: number) => {
    const o = options[i];
    if (o && o.value !== value) onChange(o.value);
    close();
  };

  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const last = options.length - 1;
    const to = { ArrowDown: Math.min(last, active + 1), ArrowUp: Math.max(0, active - 1), Home: 0, End: last }[e.key];
    if (to !== undefined) {
      e.preventDefault();
      setActive(to);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      choose(active);
    } else if (e.key === "Tab") {
      close();
    } else if (e.key.length === 1 && /\S/.test(e.key)) {
      const k = e.key.toLowerCase();
      const next = [...options.keys()].map((i) => (active + 1 + i) % options.length).find((i) => options[i].label.toLowerCase().startsWith(k));
      if (next !== undefined) setActive(next);
    }
  };

  return (
    <>
      <button
        type="button"
        className={`cmp-pick-btn cmp-select-btn${className ? ` ${className}` : ""}`}
        aria-haspopup="listbox"
        aria-expanded={anchor !== null}
        aria-label={`${label}: ${current?.label ?? placeholder ?? ""}`}
        onClick={(e) => (anchor ? close() : open(e.currentTarget))}
        onKeyDown={(e) => {
          if (!anchor && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
            e.preventDefault();
            open(e.currentTarget);
          }
        }}
      >
        {current ? <span>{current.render ?? current.label}</span> : <span className="cmp-select-placeholder">{placeholder}</span>}
        <ChevronDownIcon size={14} />
      </button>
      {anchor && (
        <Popover anchor={anchor} onClose={close} width={Math.max(168, anchor.offsetWidth)} label={label} className="cmp-select-pop">
          <div ref={list} id={id} role="listbox" aria-label={label} className="cmp-options" onKeyDown={onKey}>
            {options.map((o, i) => (
              <div
                key={String(o.value)}
                role="option"
                data-index={i}
                aria-selected={i === chosen && current !== null}
                tabIndex={i === active ? 0 : -1}
                className={i === active ? "cmp-option is-active" : "cmp-option"}
                onMouseEnter={() => setActive(i)}
                onClick={() => choose(i)}
              >
                <span className="cmp-option-label">{o.render ?? o.label}</span>
                {i === chosen && current !== null && <TickIcon size={14} />}
              </div>
            ))}
          </div>
        </Popover>
      )}
    </>
  );
}
