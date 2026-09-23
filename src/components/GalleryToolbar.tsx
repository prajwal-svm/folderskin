import { type ReactNode, useEffect, useLayoutEffect, useRef, useState } from "react";
import { SearchIcon } from "./icons/search";

export type TabCount = { id: string; label: string; count: number };

/**
 * Tag filters and search. "All" comes first, then one filter per tag, most used first. The
 * active filter is a raised pill that slides between them, and Cmd/Ctrl+F jumps to the search.
 */
export function GalleryToolbar({
  tabs,
  active,
  onChange,
  query,
  onQuery,
  label = "filter skins by tag",
  placeholder = "Search skins",
  extra,
}: {
  tabs: TabCount[];
  active: string;
  onChange: (id: string) => void;
  query: string;
  onQuery: (q: string) => void;
  label?: string;
  placeholder?: string;
  /** More controls after the search, such as how to show what's listed. */
  extra?: ReactNode;
}) {
  const seg = useRef<HTMLDivElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const [pill, setPill] = useState<{ x: number; w: number } | null>(null);
  const [animate, setAnimate] = useState(false);

  useLayoutEffect(() => {
    const measure = () => {
      const el = seg.current?.querySelector<HTMLElement>(`[data-tab="${CSS.escape(active)}"]`);
      setPill(el ? { x: el.offsetLeft, w: el.offsetWidth } : null);
      el?.scrollIntoView({ inline: "nearest", block: "nearest", behavior: "smooth" });
    };
    measure();
    const ro = new ResizeObserver(measure);
    if (seg.current) ro.observe(seg.current);
    return () => ro.disconnect();
  }, [active, tabs]);

  // Slide only after the first placement, so the pill doesn't fly in from the left on load.
  useEffect(() => {
    if (pill && !animate) requestAnimationFrame(() => setAnimate(true));
  }, [pill, animate]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        search.current?.focus();
        search.current?.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const mac = navigator.platform.toLowerCase().includes("mac");

  return (
    <div className={extra ? "toolbar has-extra" : "toolbar"} data-tauri-drag-region>
      <div className="seg" role="tablist" aria-label={label} ref={seg}>
        {pill && (
          <span
            className="seg-pill"
            aria-hidden="true"
            style={{
              width: pill.w,
              transform: `translateX(${pill.x}px)`,
              transition: animate ? undefined : "none",
            }}
          />
        )}
        {tabs.map((t) => (
          <button
            key={t.id}
            data-tab={t.id}
            type="button"
            role="tab"
            aria-selected={t.id === active}
            className={t.id === active ? "seg-btn is-active" : "seg-btn"}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => onChange(t.id)}
          >
            {t.label}
            <span className="count">{t.count}</span>
          </button>
        ))}
      </div>
      <label className={query ? "search has-query" : "search"}>
        <SearchIcon size={15} />
        <input
          ref={search}
          type="search"
          value={query}
          placeholder={placeholder}
          aria-label={placeholder.toLowerCase()}
          spellCheck={false}
          onChange={(e) => onQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              onQuery("");
              e.currentTarget.blur();
            }
          }}
        />
        {!query && <kbd>{mac ? "⌘F" : "Ctrl F"}</kbd>}
      </label>
      {extra}
    </div>
  );
}
