import { memo, useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import { Modal } from "./Modal";
import { api, errorMessage } from "../lib/tauri";
import { byName, check, everything, isEverything, isNothing, pathIn, setAll, takes, toggle, type Check, type Choice } from "../lib/folderChoice";
import { visibleRange } from "../lib/virtual";
import { clip } from "../lib/names";
import { useChoiceCount } from "../hooks/useChoiceCount";
import { INTL_LOCALES, useLocale, useT } from "../i18n";
import { reducesMotion } from "../state/prefs";
import type { Folder, SubfolderChoice } from "../state/dropzone";
import { LoaderIcon } from "./icons/loader";
import { ChevronRightIcon, FolderIcon } from "./icons/composer";
import "../styles/chooser.css";

/** A row's height, in px. Every row is the same, so a column of thousands is laid out by arithmetic. */
const ROW = 28;
/** Space above the first row and below the last, in px. */
const PAD = 6;
/** Rows drawn above and below the ones in view, so a quick scroll doesn't show blank rows. */
const OVERSCAN = 8;
/** How long the letters typed to find a folder by name wait for the next one. */
const TYPE_AHEAD_MS = 900;
/** How long the columns take to glide along to a new one. */
const GLIDE_MS = 200;

/** One folder's own folders, as a column shows them: in Finder's order, and where each name is. */
type Ready = {
  state: "ready";
  names: string[];
  /** Whether each has folders inside it: null when that wasn't looked at. */
  nested: (boolean | null)[];
  at: Map<string, number>;
};
type Listing = { state: "loading" } | { state: "failed"; message: string } | Ready;

/**
 * "Choose subfolders": which folders inside the chosen one "Include subfolders" takes, laid out
 * as Finder's column view. The first column is the folders directly inside it. Clicking a folder
 * shows the folders inside that one in the next column, and so on down, five columns across
 * before the row of them scrolls sideways, keeping the newest in view. A path bar under them says
 * where the highlighted folder is.
 *
 * Each folder has a box that stands for it and everything inside it: ticked, clear, or a dash when
 * only some are ticked. Clicking the box ticks or clears without moving, and clicking the row
 * moves to it. The keys are Finder's: up and down within a column, right into a folder, left back
 * out, Home and End, and typing a name to jump to it. Space ticks, Return is Done, Escape cancels.
 *
 * Nothing reads the whole tree up front: each column reads only its own folder's folders as it
 * opens, so a folder with a million folders inside opens at once, and each draws only the rows in
 * view, however many it holds. The ticks are rules (lib/folderChoice.ts), and how many folders
 * they come to fills in as the background count gets to the folders they're on.
 */
export function SubfolderChooser({
  folder,
  chosen,
  folderIcon,
  onDone,
  onCancel,
}: {
  folder: Folder;
  /** The folders chosen before, ticked again. Null ticks every one. */
  chosen: SubfolderChoice | null;
  /** The plain folder the way the app draws it, for every folder in the list. */
  folderIcon: string | null;
  /** Done: the folders ticked, or null when every one inside is. */
  onDone: (chosen: SubfolderChoice | null) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const locale = useLocale();
  const id = useId();
  /** The folder the chooser is for, as a run names it, once its own folders have been read. */
  const [top, setTop] = useState<{ path: string; separator: string } | null>(null);
  const [topFailed, setTopFailed] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [lists, setLists] = useState<ReadonlyMap<string, Listing>>(new Map());
  const [choice, setChoice] = useState<Choice | null>(null);
  /** The highlighted folders, one per column from the first: each is inside the one before. */
  const [trail, setTrail] = useState<string[]>([]);
  const strip = useRef<HTMLDivElement>(null);
  const before = useRef(chosen);
  const typed = useRef({ text: "", at: 0 });
  /** ArrowRight pressed on a folder whose folders were still being read: go in once they are. */
  const goIn = useRef<{ from: string; times: number } | null>(null);

  // A language chosen while the dialog is open sorts the columns read after it.
  const collator = useMemo(() => byName(INTL_LOCALES[locale]), [locale]);
  const collatorNow = useRef(collator);
  collatorNow.current = collator;

  /** What reading `path`'s own folders found, in Finder's order. */
  const ready = useCallback((names: string[], nested: (boolean | null)[]): Ready => {
    const compare = collatorNow.current.compare;
    const order = names.map((_, i) => i);
    // The same name spelled differently only in ways the language ignores still has one order.
    order.sort((a, b) => compare(names[a], names[b]) || (names[a] < names[b] ? -1 : names[a] > names[b] ? 1 : a - b));
    const sorted = order.map((i) => names[i]);
    return { state: "ready", names: sorted, nested: order.map((i) => nested[i] ?? null), at: new Map(sorted.map((name, i) => [name, i])) };
  }, []);

  const put = useCallback((path: string, listing: Listing) => setLists((lists) => new Map(lists).set(path, listing)), []);

  /** The folders whose own folders have been asked for, so each column is read once. */
  const asked = useRef(new Set<string>());

  /** Reads the folders inside `path` for its column, unless they're read or on their way (`again` reads them anyway). */
  const read = useCallback(
    (path: string, again = false) => {
      if (asked.current.has(path) && !again) return;
      asked.current.add(path);
      put(path, { state: "loading" });
      api
        .subfolderList(path)
        .then((list) => put(path, ready(list.names, list.nested)))
        .catch((e) => put(path, { state: "failed", message: errorMessage(e) }));
    },
    [put, ready],
  );

  // The folder's own folders, read as the dialog opens, and again when asked to try again.
  useEffect(() => {
    let live = true;
    setTopFailed(null);
    api
      .subfolderList(folder.path)
      .then((list) => {
        if (!live) return;
        const earlier = before.current;
        setChoice(earlier && earlier.choice.root === list.path ? earlier.choice : everything(list.path, list.separator));
        const listing = ready(list.names, list.nested);
        asked.current = new Set([list.path]);
        setLists(new Map([[list.path, listing]]));
        setTop({ path: list.path, separator: list.separator });
        const first = listing.names[0];
        setTrail(first === undefined ? [] : [list.path + list.separator + first]);
      })
      .catch((e) => live && setTopFailed(t("folder.choose.failed", { reason: errorMessage(e) })));
    return () => {
      live = false;
    };
    // Read again only when asked to try again.
  }, [folder.path, attempt]);

  // The highlighted folder's own folders, for the column after it.
  const current = trail.length > 0 ? trail[trail.length - 1] : undefined;
  useEffect(() => {
    if (current !== undefined) read(current);
  }, [current, read]);

  // ArrowRight on a folder still being read goes in once it's read, if it's still the one
  // highlighted, and so does each ArrowRight pressed while it waited, one level at a time.
  useEffect(() => {
    const waiting = goIn.current;
    if (waiting === null || !top) return;
    // The highlight has moved on meanwhile: that ArrowRight is forgotten.
    if (waiting.from !== current) {
      goIn.current = null;
      return;
    }
    const listing = lists.get(waiting.from);
    if (!listing || listing.state === "loading") return;
    goIn.current = null;
    if (listing.state !== "ready" || listing.names.length === 0) return;
    const next = pathIn(top, waiting.from, listing.names[0]);
    if (waiting.times > 1) goIn.current = { from: next, times: waiting.times - 1 };
    setTrail((trail) => [...trail, next]);
  }, [lists, current, top]);

  const counted = useChoiceCount(top ? folder.path : null, choice);

  /** Each column: the folder whose insides it lists, and which of them is highlighted. */
  const columns = useMemo(() => {
    if (!top) return [];
    const parents = [top.path, ...trail];
    return parents.map((parent, i) => ({ parent, listing: lists.get(parent), open: trail[i] ?? null }));
  }, [top, trail, lists]);
  const active = Math.max(0, trail.length - 1);
  const topListing = top ? lists.get(top.path) : undefined;
  const empty = topListing?.state === "ready" && topListing.names.length === 0;

  const pick = useCallback((column: number, path: string) => setTrail((trail) => [...trail.slice(0, column), path]), []);
  // A click on a column's empty space leaves nothing highlighted in it, as in Finder: the folder
  // it lists is where the keys are.
  const blank = useCallback((column: number) => setTrail((trail) => trail.slice(0, column)), []);
  const rowId = useCallback((column: number, row: number) => `${id}-c${column}-r${row}`, [id]);
  const tick = useCallback((path: string) => setChoice((choice) => (choice ? toggle(choice, path) : choice)), []);
  const tickAll = (on: boolean) => setChoice((choice) => (choice ? setAll(choice, on) : choice));

  const done = () => {
    if (!choice) return;
    const count = counted ?? { count: 0, total: 0, done: false };
    const all = isEverything(choice) || (count.done && count.count === count.total && !isNothing(choice));
    onDone(all ? null : { choice, ...count });
  };

  // The newest column in view, whole, and the highlighted one too: going deeper than five columns
  // glides the row of them along, as Finder does. A glide already under way is aimed again rather
  // than started over, so a held arrow key keeps up however deep it goes.
  const glide = useRef({ frame: 0, to: -1 });
  useLayoutEffect(() => {
    const el = strip.current;
    if (!el || !top) return;
    const cols = el.querySelectorAll<HTMLElement>(":scope > .fsc-col");
    const last = cols[cols.length - 1];
    if (!last) return;
    let left = glide.current.to >= 0 ? glide.current.to : el.scrollLeft;
    const end = last.offsetLeft + last.offsetWidth;
    if (end > left + el.clientWidth) left = end - el.clientWidth;
    const shown = cols[active];
    if (shown && shown.offsetLeft < left) left = shown.offsetLeft;
    left = Math.max(0, Math.min(left, el.scrollWidth - el.clientWidth));
    if (Math.abs(left - el.scrollLeft) <= 1) return;
    cancelAnimationFrame(glide.current.frame);
    if (reducesMotion()) {
      el.scrollLeft = left;
      glide.current.to = -1;
      return;
    }
    const from = el.scrollLeft;
    const started = performance.now();
    glide.current.to = left;
    const step = (now: number) => {
      const done = Math.min(1, (now - started) / GLIDE_MS);
      el.scrollLeft = from + (left - from) * (1 - (1 - done) ** 3);
      if (done < 1) glide.current.frame = requestAnimationFrame(step);
      else glide.current.to = -1;
    };
    glide.current.frame = requestAnimationFrame(step);
  }, [top, columns.length, active]);
  useEffect(() => () => cancelAnimationFrame(glide.current.frame), []);

  // Once the folders are in, the keyboard is in them too, unless it has gone elsewhere meanwhile.
  useEffect(() => {
    if (!top) return;
    const el = strip.current;
    if (el && (document.activeElement === document.body || el.contains(document.activeElement))) el.focus();
  }, [top]);

  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (!top || !choice || e.metaKey || e.ctrlKey || e.altKey) return;
    const column = columns[active];
    const listing = column?.listing;
    if (!column || listing?.state !== "ready") return;
    const names = listing.names;
    const at = current === undefined ? -1 : (listing.at.get(current.slice(column.parent.length + top.separator.length)) ?? -1);
    const page = Math.max(1, Math.floor(((strip.current?.clientHeight ?? 300) - PAD * 2) / ROW) - 1);
    const go = (i: number) => {
      const name = names[Math.min(names.length - 1, Math.max(0, i))];
      if (name !== undefined) pick(active, pathIn(top, column.parent, name));
    };
    switch (e.key) {
      case "ArrowDown":
        go(at + 1);
        break;
      case "ArrowUp":
        go(at < 0 ? 0 : at - 1);
        break;
      case "Home":
        go(0);
        break;
      case "End":
        go(names.length - 1);
        break;
      case "PageDown":
        go(at + page);
        break;
      case "PageUp":
        go(at - page);
        break;
      case "ArrowRight": {
        if (current === undefined) break;
        // Already waiting to go in: one more level once that's done.
        if (goIn.current) {
          goIn.current = { ...goIn.current, times: goIn.current.times + 1 };
          break;
        }
        const inside = lists.get(current);
        if (inside?.state === "ready") {
          if (inside.names.length > 0) setTrail([...trail, pathIn(top, current, inside.names[0])]);
        } else if (inside?.state !== "failed") {
          goIn.current = { from: current, times: 1 };
        }
        break;
      }
      case "ArrowLeft":
        goIn.current = null;
        if (trail.length > 1) setTrail(trail.slice(0, -1));
        break;
      case " ":
        if (current !== undefined) tick(current);
        break;
      case "Enter":
        done();
        break;
      default: {
        // Typing a name jumps to the first folder in the column that starts with it, as in Finder.
        if (e.key.length !== 1) return;
        const now = performance.now();
        const text = (now - typed.current.at < TYPE_AHEAD_MS ? typed.current.text : "") + e.key.toLocaleLowerCase();
        typed.current = { text, at: now };
        const found = names.findIndex((name) => name.toLocaleLowerCase().startsWith(text));
        if (found >= 0) go(found);
        break;
      }
    }
    e.preventDefault();
  };

  const name = clip(folder.name);
  const keep = (e: MouseEvent) => e.preventDefault();
  const loading = !top && topFailed === null;
  const currentRow = current === undefined || !top ? undefined : rowOf(columns[active], current, top.separator);

  return (
    <Modal
      title={t("folder.choose.title")}
      sub={t("folder.choose.sub", { name })}
      className="modal-chooser"
      onClose={onCancel}
      footer={
        <>
          <p className="fsc-count" aria-live="polite">
            {choice && counted ? countLine(t, counted) : ""}
          </p>
          <button type="button" className="btn btn-ghost btn-sm" disabled={!choice || isEverything(choice)} onMouseDown={keep} onClick={() => tickAll(true)}>
            {t("folder.choose.all")}
          </button>
          <button type="button" className="btn btn-ghost btn-sm" disabled={!choice || isNothing(choice)} onMouseDown={keep} onClick={() => tickAll(false)}>
            {t("folder.choose.none")}
          </button>
          <span className="fsc-foot-gap" />
          <button type="button" className="btn btn-ghost" onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <button type="button" className="btn btn-primary" disabled={!choice} onClick={done}>
            {t("folder.choose.done")}
          </button>
        </>
      }
    >
      <div className="fsc">
        <div
          ref={strip}
          className="fsc-strip"
          role="tree"
          aria-label={t("folder.choose.treeLabel", { name })}
          aria-busy={loading || undefined}
          aria-activedescendant={currentRow !== undefined ? rowId(active, currentRow) : undefined}
          tabIndex={0}
          data-modal-focus
          onKeyDown={onKey}
        >
          {loading && (
            <p className="fsc-status" role="status">
              <LoaderIcon size={16} />
              {t("folder.choose.loading")}
            </p>
          )}
          {topFailed !== null && (
            <div className="fsc-status is-failed" role="alert">
              <p>{topFailed}</p>
              <button type="button" className="btn btn-secondary btn-sm" onClick={() => setAttempt((a) => a + 1)}>
                {t("folder.choose.tryAgain")}
              </button>
            </div>
          )}
          {empty && <p className="fsc-status">{t("folder.choose.empty")}</p>}
          {top &&
            choice &&
            !empty &&
            columns.map((column, i) => {
              const listing = column.listing;
              if (!listing || listing.state === "loading") return <Pending key={column.parent} />;
              if (listing.state === "failed") {
                return <Failed key={column.parent} message={t("folder.choose.failed", { reason: listing.message })} onRetry={() => read(column.parent, true)} />;
              }
              if (listing.names.length === 0) return <Leaf key={column.parent} name={lastPart(column.parent, top.separator)} chosen={takes(choice, column.parent)} icon={folderIcon} />;
              return (
                <Column
                  key={column.parent}
                  choice={choice}
                  index={i}
                  parent={column.parent}
                  name={lastPart(column.parent, top.separator)}
                  listing={listing}
                  open={column.open}
                  current={i === active}
                  level={i + 1}
                  icon={folderIcon}
                  rowId={rowId}
                  onPick={pick}
                  onBlank={blank}
                  onTick={tick}
                />
              );
            })}
        </div>
        {top && !empty && (
          <nav className="fsc-path" aria-label={t("folder.choose.pathLabel")}>
            <PathBar
              steps={[top.path, ...trail]}
              separator={top.separator}
              icon={folderIcon}
              onGo={(depth) => {
                goIn.current = null;
                setTrail(trail.slice(0, depth));
                strip.current?.focus();
              }}
            />
          </nav>
        )}
      </div>
    </Modal>
  );
}

/** The footer's count: "22 of 28 folders chosen", or how far the count has got while it's going. */
function countLine(t: ReturnType<typeof useT>, counted: { count: number; total: number; done: boolean }): string {
  if (counted.done) return t("folder.choose.count", { count: counted.count, total: counted.total });
  if (counted.total === 0) return t("folder.stage.counting");
  return t("folder.choose.countingSoFar", { count: counted.count, total: counted.total });
}

/** The last part of a folder's path: its name. */
const lastPart = (path: string, separator: string) => path.slice(path.lastIndexOf(separator) + separator.length) || path;

/** Where the folder at `path` is in `column`'s rows, when it's listed there. */
function rowOf(column: { parent: string; listing?: Listing } | undefined, path: string, separator: string): number | undefined {
  if (column?.listing?.state !== "ready" || !path.startsWith(column.parent + separator)) return undefined;
  return column.listing.at.get(path.slice(column.parent.length + separator.length));
}

/** The folder's picture: the app's own plain folder, or its outline until that's drawn. */
function Icon({ src, className }: { src: string | null; className: string }) {
  return src ? (
    <img className={className} src={src} alt="" draggable={false} />
  ) : (
    <span className={className}>
      <FolderIcon size={16} />
    </span>
  );
}

/** A folder's box: ticked, clear or a dash. */
function Box({ check }: { check: Check }) {
  return (
    <span className={`fsc-box is-${check}`}>
      {check === "on" && (
        <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
          <path d="M2.5 6.2 4.9 8.6 9.5 3.6" />
        </svg>
      )}
      {check === "mixed" && (
        <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
          <path d="M3 6h6" />
        </svg>
      )}
    </span>
  );
}

/**
 * One column: the folders directly inside `parent`, in Finder's order. Only the rows in view are
 * drawn, and the highlighted one wherever it is, so the keyboard never loses its place. The rest
 * are a spacer as tall as they would be, so the scroll bar is honest.
 */
const Column = memo(function Column({
  choice,
  index,
  parent,
  name,
  listing,
  open,
  current,
  level,
  icon,
  rowId,
  onPick,
  onBlank,
  onTick,
}: {
  choice: Choice;
  index: number;
  parent: string;
  /** The folder it lists the folders of. */
  name: string;
  listing: Ready;
  /** The highlighted folder in it, or null. */
  open: string | null;
  /** It holds the folder the keys move from. */
  current: boolean;
  /** How far down its folders are: 1 for the folders directly inside the chosen one. */
  level: number;
  icon: string | null;
  rowId: (column: number, row: number) => string;
  onPick: (column: number, path: string) => void;
  /** Its empty space was clicked. */
  onBlank: (column: number) => void;
  onTick: (path: string) => void;
}) {
  const scroller = useRef<HTMLDivElement>(null);
  const [top, setTop] = useState(0);
  const [height, setHeight] = useState(0);
  const frame = useRef(0);

  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const measure = () => setHeight(el.clientHeight);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => {
      ro.disconnect();
      cancelAnimationFrame(frame.current);
    };
  }, []);

  // The highlighted folder in view when the keys move it: just far enough, as a list scrolls.
  const openAt = open === null ? -1 : (listing.at.get(open.slice(parent.length + choice.separator.length)) ?? -1);
  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el || openAt < 0) return;
    const y = PAD + openAt * ROW;
    let next = el.scrollTop;
    if (y - PAD < next) next = y - PAD;
    else if (y + ROW + PAD > next + el.clientHeight) next = y + ROW + PAD - el.clientHeight;
    if (next !== el.scrollTop) {
      el.scrollTop = next;
      setTop(el.scrollTop);
    }
  }, [openAt]);

  const onScroll = () => {
    cancelAnimationFrame(frame.current);
    frame.current = requestAnimationFrame(() => setTop(scroller.current?.scrollTop ?? 0));
  };

  const count = listing.names.length;
  const { start, end } = visibleRange({ columns: 1, cellWidth: 0, rowHeight: ROW, gap: 0, rows: count, height: count * ROW }, count, top - PAD, height, OVERSCAN);
  const drawn: number[] = [];
  if (openAt >= 0 && (openAt < start || openAt >= end)) drawn.push(openAt);
  for (let i = start; i < end; i++) drawn.push(i);

  return (
    <div
      ref={scroller}
      className="fsc-col"
      role="group"
      aria-label={name}
      onScroll={onScroll}
      onMouseDown={(e) => {
        // Not a row, and not the scroll bar beside them.
        const el = e.currentTarget;
        if (e.button === 0 && !(e.target as Element).closest(".fsc-row") && e.clientX < el.getBoundingClientRect().left + el.clientWidth) onBlank(index);
      }}
    >
      <div className="fsc-space" style={{ height: count * ROW + PAD * 2 }}>
        {drawn.map((i) => {
          const folderName = listing.names[i];
          const path = pathIn(choice, parent, folderName);
          const box = check(choice, path);
          const isOpen = path === open;
          // Unknown until it's opened: maybe there are folders inside.
          const hasKids = listing.nested[i] !== false;
          const cls = ["fsc-row", isOpen && (current ? "is-current" : "is-trail"), !takes(choice, path) && "is-out"].filter(Boolean).join(" ");
          return (
            <div
              key={folderName}
              id={rowId(index, i)}
              className={cls}
              role="treeitem"
              aria-level={level}
              aria-setsize={count}
              aria-posinset={i + 1}
              aria-checked={box === "mixed" ? "mixed" : box === "on"}
              aria-expanded={hasKids ? isOpen : undefined}
              style={{ transform: `translateY(${PAD + i * ROW}px)` }}
              onMouseDown={(e) => {
                if (e.button === 0) onPick(index, path);
              }}
            >
              <span
                className="fsc-hit"
                aria-hidden="true"
                onMouseDown={(e) => e.stopPropagation()}
                onClick={(e) => {
                  e.stopPropagation();
                  onTick(path);
                }}
              >
                <Box check={box} />
              </span>
              <Icon src={icon} className="fsc-icon" />
              <span className="fsc-name" data-tip={folderName} data-tip-overflow>
                {folderName}
              </span>
              {hasKids && (
                <span className="fsc-chev" aria-hidden="true">
                  <ChevronRightIcon size={13} />
                </span>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
});

/** The column after a folder with none inside it: the folder itself, large, as Finder previews a file. */
function Leaf({ name, chosen, icon }: { name: string; chosen: boolean; icon: string | null }) {
  const t = useT();
  return (
    <div className="fsc-col fsc-leaf" aria-hidden="true">
      <Icon src={icon} className={chosen ? "fsc-leaf-icon" : "fsc-leaf-icon is-out"} />
      <p className="fsc-leaf-name">{name}</p>
      <p className="fsc-leaf-note">{t("folder.choose.empty")}</p>
    </div>
  );
}

/** A column whose folders are being read: nothing at first, and a spinner if it takes a moment. */
function Pending() {
  const t = useT();
  return (
    <div className="fsc-col fsc-pending" role="status">
      <LoaderIcon size={15} />
      <span>{t("folder.choose.loading")}</span>
    </div>
  );
}

/** A column whose folders couldn't be read, and a way to try again. */
function Failed({ message, onRetry }: { message: string; onRetry: () => void }) {
  const t = useT();
  return (
    <div className="fsc-col fsc-pending is-failed" role="alert">
      <p>{message}</p>
      <button type="button" className="btn btn-secondary btn-sm" onClick={onRetry}>
        {t("folder.choose.tryAgain")}
      </button>
    </div>
  );
}

/** Where the highlighted folder is, from the folder the dialog is for: each step goes back to it. */
function PathBar({ steps, separator, icon, onGo }: { steps: string[]; separator: string; icon: string | null; onGo: (depth: number) => void }) {
  // A long way down shows the first folder, a gap, and the last few, as there's room for.
  const LAST = 4;
  const cut = steps.length > LAST + 2 ? steps.length - LAST : 1;
  const shown = steps.map((path, depth) => ({ path, depth })).filter(({ depth }) => depth === 0 || depth >= cut);
  return (
    <ol className="fsc-path-list">
      {shown.map(({ path, depth }, i) => {
        const name = lastPart(path, separator);
        return (
          <li key={path} className={depth === steps.length - 1 ? "fsc-step is-here" : "fsc-step"}>
            {i > 0 && (
              <span className="fsc-step-sep" aria-hidden="true">
                <ChevronRightIcon size={12} />
              </span>
            )}
            {i === 1 && cut > 1 && (
              <>
                <span className="fsc-step-gap" aria-hidden="true">
                  …
                </span>
                <span className="fsc-step-sep" aria-hidden="true">
                  <ChevronRightIcon size={12} />
                </span>
              </>
            )}
            <button
              type="button"
              className="fsc-step-btn"
              aria-current={depth === steps.length - 1 ? "location" : undefined}
              data-tip={name}
              data-tip-overflow
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => onGo(depth)}
            >
              <Icon src={icon} className="fsc-step-icon" />
              <span className="fsc-step-name">{name}</span>
            </button>
          </li>
        );
      })}
    </ol>
  );
}
