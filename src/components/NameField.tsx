import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { cleanName, MAX_NAME_CHARS } from "../lib/names";

/** How a rename ended: Return, Escape, or focus going somewhere else. */
export type NameFieldClose = "enter" | "escape" | "away";

/**
 * The text field a skin's name turns into while it is renamed. It opens with the name selected.
 * Return or clicking away keeps what was typed, Escape keeps the old name, and a blank or
 * unchanged name changes nothing. If the field disappears some other way, say because the list
 * around it changed, what was typed is kept as well, the way Finder keeps a rename when you
 * click elsewhere.
 */
export function NameField({
  value,
  label,
  className,
  onRename,
  onClose,
}: {
  value: string;
  label: string;
  className?: string;
  onRename: (name: string) => void;
  onClose: (how: NameFieldClose) => void;
}) {
  const [draft, setDraft] = useState(value);
  const input = useRef<HTMLInputElement>(null);
  const open = useRef(true);
  const mounted = useRef(false);
  const latest = useRef({ draft, value, onRename, onClose });

  useLayoutEffect(() => {
    latest.current = { draft, value, onRename, onClose };
  });

  const finish = useCallback((how: NameFieldClose) => {
    if (!open.current) return;
    open.current = false;
    const { draft, value, onRename, onClose } = latest.current;
    const name = cleanName(draft);
    if (how !== "escape" && name && name !== value) onRename(name);
    onClose(how);
  }, []);

  useLayoutEffect(() => {
    mounted.current = true;
    input.current?.focus({ preventScroll: true });
    input.current?.select();
    return () => {
      mounted.current = false;
      // StrictMode unmounts and mounts once more in development; only a real unmount is still
      // unmounted a moment later.
      queueMicrotask(() => {
        if (!mounted.current) finish("away");
      });
    };
  }, [finish]);

  return (
    <input
      ref={input}
      className={className ? `name-field ${className}` : "name-field"}
      value={draft}
      maxLength={MAX_NAME_CHARS}
      aria-label={label}
      spellCheck={false}
      autoComplete="off"
      onChange={(e) => setDraft(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          finish("enter");
        } else if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          finish("escape");
        }
      }}
      onBlur={() => finish("away")}
    />
  );
}
