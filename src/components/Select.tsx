import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { ChevronDownIcon, TickIcon } from "./icons/composer";
import { Popover } from "./composer/Popover";
import { branded } from "./Brand";

export type SelectOption<T> = {
  value: T;
  label: string;
  /** Drawn instead of the label, in the list and the button. */
  render?: ReactNode;
  /** A picture of it, beside its name in the list and in the button: `null` while it's on its way. */
  picture?: string | null;
  /** The heading it's listed under; options with the same one are listed together under it. */
  group?: string;
  /** Its name in the list, under its group's heading, where the button says it in full. */
  listLabel?: string;
};

/** A picture of an option that hasn't arrived yet: the place it will take, left clear. */
const PictureOf = ({ src, className }: { src: string | null; className: string }) =>
  src ? <img className={className} src={src} alt="" draggable={false} /> : <span className={className} aria-hidden="true" />;

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
  width,
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
  /** How wide the list is at least, where its names are longer than the button. */
  width?: number;
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
      const next = [...options.keys()]
        .map((i) => (active + 1 + i) % options.length)
        .find((i) => (options[i].listLabel ?? options[i].label).toLowerCase().startsWith(k));
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
        {current && current.picture !== undefined && <PictureOf src={current.picture} className="cmp-select-pic" />}
        {current ? <span>{branded(current.render ?? current.label)}</span> : <span className="cmp-select-placeholder">{placeholder}</span>}
        <ChevronDownIcon size={14} />
      </button>
      {anchor && (
        <Popover anchor={anchor} onClose={close} width={Math.max(width ?? 168, anchor.offsetWidth)} label={label} className="cmp-select-pop">
          <div ref={list} id={id} role="listbox" aria-label={label} className="cmp-options" onKeyDown={onKey}>
            {groupsOf(options).map(({ group, entries }) => {
              const rows = entries.map(({ option: o, index: i }) => (
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
                  {o.picture !== undefined && <PictureOf src={o.picture} className="cmp-option-pic" />}
                  <span className="cmp-option-label">{branded(o.render ?? o.listLabel ?? o.label)}</span>
                  {i === chosen && current !== null && <TickIcon size={14} />}
                </div>
              ));
              if (group === undefined) return rows;
              const heading = `${id}-${group}`;
              return (
                <div key={heading} role="group" aria-labelledby={heading} className="cmp-option-group">
                  <div id={heading} className="cmp-option-heading" role="presentation">
                    {group}
                  </div>
                  {rows}
                </div>
              );
            })}
          </div>
        </Popover>
      )}
    </>
  );
}

/** `options` in runs that share a group, each option with its place in the whole list. */
function groupsOf<T>(options: SelectOption<T>[]): { group: string | undefined; entries: { option: SelectOption<T>; index: number }[] }[] {
  const runs: { group: string | undefined; entries: { option: SelectOption<T>; index: number }[] }[] = [];
  options.forEach((option, index) => {
    const last = runs[runs.length - 1];
    if (last && last.group === option.group) last.entries.push({ option, index });
    else runs.push({ group: option.group, entries: [{ option, index }] });
  });
  return runs;
}
