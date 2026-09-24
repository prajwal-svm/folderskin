import { useEffect, useMemo, useRef, useState } from "react";
import type { Skin } from "../../lib/tauri";
import { groupChats, picturesMade, searchChats, type ChatSummary } from "../../state/chats";
import { SparklesIcon } from "../icons/sparkles";
import { SearchIcon } from "../icons/search";
import { PencilIcon } from "../icons/pencil";
import { SquarePenIcon, TrashIcon, XIcon } from "../icons/composer";

/** "2:14 pm" today, "Mon" this week, "12 Sep" before that. */
function when(ms: number, now: number): string {
  const d = new Date(ms);
  const today = new Date(now);
  if (d.toDateString() === today.toDateString()) return d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  if (now - ms < 6 * 24 * 60 * 60 * 1000) return d.toLocaleDateString([], { weekday: "short" });
  return d.toLocaleDateString([], { day: "numeric", month: "short" });
}

/**
 * Every chat, newest first and grouped by day, in a panel that slides over the chat from its left
 * edge: search them, open one, start a new one, rename one or delete one. Escape or a click outside
 * closes it; Escape in a search with words in it empties the search first.
 */
export function ChatDrawer({
  open,
  list,
  activeId,
  skinOf,
  onOpen,
  onNew,
  onRename,
  onDelete,
  onClose,
}: {
  open: boolean;
  list: ChatSummary[];
  activeId: string | null;
  skinOf: (id: string) => Skin | undefined;
  onOpen: (id: string) => void;
  onNew: () => void;
  onRename: (id: string, title: string) => void;
  onDelete: (chat: ChatSummary) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [renaming, setRenaming] = useState<string | null>(null);
  const panel = useRef<HTMLElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const now = Date.now();
  const groups = useMemo(() => groupChats(searchChats(list, query), now), [list, query, now]);

  // The search has the keyboard as the drawer opens (and only then: not whenever a rename ends).
  useEffect(() => {
    if (open) search.current?.focus({ preventScroll: true });
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || renaming || document.querySelector(".modal-backdrop")) return;
      e.stopPropagation();
      // The search with words in it empties itself first, as it does in a dialog.
      if (e.target === search.current && search.current?.value) {
        e.preventDefault();
        setQuery("");
        return;
      }
      onClose();
    };
    const down = (e: MouseEvent) => {
      const t = e.target as Element;
      if (panel.current?.contains(t) || t.closest?.(".studio-chats-btn, .modal-backdrop")) return;
      onClose();
    };
    document.addEventListener("keydown", key, true);
    document.addEventListener("mousedown", down);
    return () => {
      document.removeEventListener("keydown", key, true);
      document.removeEventListener("mousedown", down);
    };
  }, [open, renaming, onClose]);

  return (
    <aside ref={panel} className={open ? "chat-drawer is-open" : "chat-drawer"} aria-label="chats" aria-hidden={!open} inert={!open}>
      <div className="chat-drawer-head">
        <p className="chat-drawer-title">Chats</p>
        <button type="button" className="icon-btn" aria-label="new chat" data-tip="New chat" onClick={onNew}>
          <SquarePenIcon size={16} />
        </button>
        <button type="button" className="icon-btn" aria-label="close the chats" data-tip="Close" onClick={onClose}>
          <XIcon size={16} />
        </button>
      </div>
      <label className="search chat-search">
        <SearchIcon size={15} />
        <input ref={search} type="search" value={query} placeholder="Search chats" aria-label="search chats" spellCheck={false} onChange={(e) => setQuery(e.target.value)} />
      </label>
      <div className="chat-list">
        {groups.length === 0 && <p className="chat-empty">{query.trim() ? `No chat is called anything like “${query.trim()}”.` : "Your chats will be here, to pick up where you left off."}</p>}
        {groups.map((g) => (
          <section key={g.label} className="chat-group" aria-label={g.label}>
            <p className="chat-group-title">{g.label}</p>
            {g.chats.map((c) => {
              const cover = c.cover ? skinOf(c.cover) : undefined;
              return (
                <div key={c.id} className={c.id === activeId ? "chat-row is-on" : "chat-row"}>
                  {renaming === c.id ? (
                    <input
                      className="chat-rename"
                      defaultValue={c.title}
                      autoFocus
                      maxLength={80}
                      aria-label="chat name"
                      onFocus={(e) => e.currentTarget.select()}
                      onBlur={(e) => {
                        if (e.currentTarget.value.trim() !== c.title) onRename(c.id, e.currentTarget.value);
                        setRenaming(null);
                      }}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") e.currentTarget.blur();
                        if (e.key === "Escape") {
                          e.stopPropagation();
                          e.currentTarget.value = c.title;
                          setRenaming(null);
                        }
                      }}
                    />
                  ) : (
                    <button type="button" className="chat-open" aria-current={c.id === activeId} onClick={() => onOpen(c.id)} onDoubleClick={() => setRenaming(c.id)}>
                      <span className="chat-cover" aria-hidden="true">
                        {cover ? <img src={cover.thumbnail} alt="" draggable={false} /> : <SparklesIcon size={14} />}
                      </span>
                      <span className="chat-text">
                        <span className="chat-title" data-tip={c.title} data-tip-overflow>
                          {c.title}
                        </span>
                        <span className="chat-meta">
                          {when(c.updated, now)} · {picturesMade(c.pictures ?? 0)}
                        </span>
                      </span>
                    </button>
                  )}
                  {renaming !== c.id && (
                    <span className="chat-row-actions">
                      <button type="button" className="chat-row-btn" aria-label={`rename ${c.title}`} data-tip="Rename" onClick={() => setRenaming(c.id)}>
                        <PencilIcon size={13} />
                      </button>
                      <button type="button" className="chat-row-btn is-danger" aria-label={`delete ${c.title}`} data-tip="Delete" onClick={() => onDelete(c)}>
                        <TrashIcon size={13} />
                      </button>
                    </span>
                  )}
                </div>
              );
            })}
          </section>
        ))}
      </div>
    </aside>
  );
}
