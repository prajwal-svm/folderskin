import { memo, useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import { Modal } from "./Modal";
import { api, errorMessage } from "../lib/tauri";
import { byName, FolderChoice, type Check } from "../lib/folderChoice";
import { visibleRange } from "../lib/virtual";
import { tooMany } from "../lib/tree";
import { clip } from "../lib/names";
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

type Loaded = { state: "loading" } | { state: "failed"; message: string } | { state: "ready"; choice: FolderChoice };

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
 * The whole tree is read once, when the dialog opens: at most 5,000 folders, the most a run
 * takes. After that every column opens at once, and each draws only the rows in view, however
 * many folders it holds.
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
  /** Done: the folders ticked, or null when every one of the `total` inside is. */
  onDone: (chosen: SubfolderChoice | null, total: number) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const locale = useLocale();
  const id = useId();
  const [loaded, setLoaded] = useState<Loaded>({ state: "loading" });
  const [attempt, setAttempt] = useState(0);
  /** Bumped by every tick, so the rows on screen draw their boxes again. */
  const [version, setVersion] = useState(0);
  /** The highlighted folders, one per column from the first: each is inside the one before. */
  const [trail, setTrail] = useState<number[]>([]);
  const strip = useRef<HTMLDivElement>(null);
  const before = useRef(chosen);
  const typed = useRef({ text: "", at: 0 });

  // The tree, read once. The locale sorts names the way the language on show does.
  const collator = useMemo(() => byName(INTL_LOCALES[locale]), [locale]);
  useEffect(() => {
    let live = true;
    setLoaded({ state: "loading" });
    api
      .subfolderTree(folder.path)
      .then((tree) => {
        if (!live) return;
        if (tree.more) {
          setLoaded({ state: "failed", message: tooMany("folders") });
          return;
        }
        const choice = new FolderChoice(tree, collator);
        const earlier = before.current;
        if (earlier && earlier.root === tree.root) choice.pick(earlier.paths);
        const first = choice.children(0)[0];
        setTrail(first === undefined ? [] : [first]);
        setLoaded({ state: "ready", choice });
      })
      .catch((e) => live && setLoaded({ state: "failed", message: t("folder.choose.failed", { reason: errorMessage(e) }) }));
    return () => {
      live = false;
    };
    // Read again only when asked to try again. A language chosen meanwhile sorts the next time.
  }, [folder.path, attempt]);

  const choice = loaded.state === "ready" ? loaded.choice : null;
  const latest = useRef(choice);
  latest.current = choice;

  /** Each column: the folder whose insides it lists, and which of them is highlighted. */
  const columns = useMemo(() => {
    if (!choice) return [];
    const parents = [0, ...trail];
    return parents.map((parent, i) => ({ parent, kids: choice.children(parent), open: trail[i] ?? -1 }));
  }, [choice, trail]);
  const active = Math.max(0, trail.length - 1);
  const current = trail.length > 0 ? trail[trail.length - 1] : undefined;

  const pick = useCallback((column: number, node: number) => setTrail((trail) => [...trail.slice(0, column), node]), []);
  // A click on a column's empty space leaves nothing highlighted in it, as in Finder: the folder
  // it lists is where the keys are.
  const blank = useCallback((column: number) => setTrail((trail) => trail.slice(0, column)), []);
  const rowId = useCallback((node: number) => `${id}-f${node}`, [id]);
  const tick = useCallback((node: number) => {
    if (latest.current?.toggle(node)) setVersion((v) => v + 1);
  }, []);
  const tickAll = (on: boolean) => {
    if (!choice) return;
    choice.setAll(on);
    setVersion((v) => v + 1);
  };

  const done = () => {
    if (!choice) return;
    const all = choice.chosen === choice.total;
    onDone(all ? null : { root: choice.root, paths: choice.chosenPaths(), total: choice.total }, choice.total);
  };

  // The newest column in view, whole, and the highlighted one too: going deeper than five columns
  // glides the row of them along, as Finder does. A glide already under way is aimed again rather
  // than started over, so a held arrow key keeps up however deep it goes.
  const glide = useRef({ frame: 0, to: -1 });
  useLayoutEffect(() => {
    const el = strip.current;
    if (!el || !choice) return;
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
  }, [choice, columns.length, active]);
  useEffect(() => () => cancelAnimationFrame(glide.current.frame), []);

  // Once the folders are in, the keyboard is in them too, unless it has gone elsewhere meanwhile.
  useEffect(() => {
    if (!choice) return;
    const el = strip.current;
    if (el && (document.activeElement === document.body || el.contains(document.activeElement))) el.focus();
  }, [choice]);

  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (!choice || e.metaKey || e.ctrlKey || e.altKey) return;
    const kids = columns[active]?.kids;
    if (!kids) return;
    const at = current === undefined ? -1 : choice.indexOf(current);
    const page = Math.max(1, Math.floor(((strip.current?.clientHeight ?? 300) - PAD * 2) / ROW) - 1);
    const go = (i: number) => {
      const node = kids[Math.min(kids.length - 1, Math.max(0, i))];
      if (node !== undefined) pick(active, node);
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
        go(kids.length - 1);
        break;
      case "PageDown":
        go(at + page);
        break;
      case "PageUp":
        go(at - page);
        break;
      case "ArrowRight":
        if (current !== undefined && choice.hasChildren(current)) setTrail([...trail, choice.children(current)[0]]);
        break;
      case "ArrowLeft":
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
        const found = kids.findIndex((node) => choice.name(node).toLocaleLowerCase().startsWith(text));
        if (found >= 0) go(found);
        break;
      }
    }
    e.preventDefault();
  };

  const name = clip(folder.name);
  const keep = (e: MouseEvent) => e.preventDefault();

  return (
    <Modal
      title={t("folder.choose.title")}
      sub={t("folder.choose.sub", { name })}
      className="modal-chooser"
      onClose={onCancel}
      footer={
        <>
          <p className="fsc-count" aria-live="polite">
            {choice ? t("folder.choose.count", { count: choice.chosen, total: choice.total }) : ""}
          </p>
          <button type="button" className="btn btn-ghost btn-sm" disabled={!choice || choice.chosen === choice.total} onMouseDown={keep} onClick={() => tickAll(true)}>
            {t("folder.choose.all")}
          </button>
          <button type="button" className="btn btn-ghost btn-sm" disabled={!choice || choice.chosen === 0} onMouseDown={keep} onClick={() => tickAll(false)}>
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
          aria-busy={loaded.state === "loading" || undefined}
          aria-activedescendant={current !== undefined ? rowId(current) : undefined}
          tabIndex={0}
          data-modal-focus
          onKeyDown={onKey}
        >
          {loaded.state === "loading" && (
            <p className="fsc-status" role="status">
              <LoaderIcon size={16} />
              {t("folder.choose.loading")}
            </p>
          )}
          {loaded.state === "failed" && (
            <div className="fsc-status is-failed" role="alert">
              <p>{loaded.message}</p>
              <button type="button" className="btn btn-secondary btn-sm" onClick={() => setAttempt((a) => a + 1)}>
                {t("folder.choose.tryAgain")}
              </button>
            </div>
          )}
          {choice && choice.total === 0 && <p className="fsc-status">{t("folder.choose.empty")}</p>}
          {choice &&
            choice.total > 0 &&
            columns.map((column, i) =>
              column.kids.length > 0 ? (
                <Column
                  key={column.parent}
                  choice={choice}
                  version={version}
                  index={i}
                  parent={column.parent}
                  kids={column.kids}
                  open={column.open}
                  current={i === active}
                  icon={folderIcon}
                  rowId={rowId}
                  onPick={pick}
                  onBlank={blank}
                  onTick={tick}
                />
              ) : (
                <Leaf key={column.parent} choice={choice} node={column.parent} icon={folderIcon} />
              ),
            )}
        </div>
        {choice && choice.total > 0 && (
          <nav className="fsc-path" aria-label={t("folder.choose.pathLabel")}>
            <PathBar
              choice={choice}
              trail={trail}
              icon={folderIcon}
              onGo={(depth) => {
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
  version,
  index,
  parent,
  kids,
  open,
  current,
  icon,
  rowId,
  onPick,
  onBlank,
  onTick,
}: {
  choice: FolderChoice;
  /** Changes with every tick, so the boxes draw again. */
  version: number;
  index: number;
  parent: number;
  kids: Int32Array;
  /** The highlighted folder in it, or -1. */
  open: number;
  /** It holds the folder the keys move from. */
  current: boolean;
  icon: string | null;
  rowId: (node: number) => string;
  onPick: (column: number, node: number) => void;
  /** Its empty space was clicked. */
  onBlank: (column: number) => void;
  onTick: (node: number) => void;
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
  const openAt = open >= 0 ? choice.indexOf(open) : -1;
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

  const count = kids.length;
  const { start, end } = visibleRange({ columns: 1, cellWidth: 0, rowHeight: ROW, gap: 0, rows: count, height: count * ROW }, count, top - PAD, height, OVERSCAN);
  const drawn: number[] = [];
  if (openAt >= 0 && (openAt < start || openAt >= end)) drawn.push(openAt);
  for (let i = start; i < end; i++) drawn.push(i);
  const level = choice.depth(parent) + 1;

  return (
    <div
      ref={scroller}
      className="fsc-col"
      role="group"
      aria-label={choice.name(parent)}
      onScroll={onScroll}
      onMouseDown={(e) => {
        // Not a row, and not the scroll bar beside them.
        const el = e.currentTarget;
        if (e.button === 0 && !(e.target as Element).closest(".fsc-row") && e.clientX < el.getBoundingClientRect().left + el.clientWidth) onBlank(index);
      }}
      data-version={version}
    >
      <div className="fsc-space" style={{ height: count * ROW + PAD * 2 }}>
        {drawn.map((i) => {
          const node = kids[i];
          const check = choice.check(node);
          const isOpen = node === open;
          const hasKids = choice.hasChildren(node);
          const cls = ["fsc-row", isOpen && (current ? "is-current" : "is-trail"), !choice.isChosen(node) && "is-out"].filter(Boolean).join(" ");
          return (
            <div
              key={node}
              id={rowId(node)}
              className={cls}
              role="treeitem"
              aria-level={level}
              aria-setsize={count}
              aria-posinset={i + 1}
              aria-checked={check === "mixed" ? "mixed" : check === "on"}
              aria-expanded={hasKids ? isOpen : undefined}
              style={{ transform: `translateY(${PAD + i * ROW}px)` }}
              onMouseDown={(e) => {
                if (e.button === 0) onPick(index, node);
              }}
            >
              <span
                className="fsc-hit"
                aria-hidden="true"
                onMouseDown={(e) => e.stopPropagation()}
                onClick={(e) => {
                  e.stopPropagation();
                  onTick(node);
                }}
              >
                <Box check={check} />
              </span>
              <Icon src={icon} className="fsc-icon" />
              <span className="fsc-name" data-tip={choice.name(node)} data-tip-overflow>
                {choice.name(node)}
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
function Leaf({ choice, node, icon }: { choice: FolderChoice; node: number; icon: string | null }) {
  const t = useT();
  return (
    <div className="fsc-col fsc-leaf" aria-hidden="true">
      <Icon src={icon} className={choice.isChosen(node) ? "fsc-leaf-icon" : "fsc-leaf-icon is-out"} />
      <p className="fsc-leaf-name">{choice.name(node)}</p>
      <p className="fsc-leaf-note">{t("folder.choose.empty")}</p>
    </div>
  );
}

/** Where the highlighted folder is, from the folder the dialog is for: each step goes back to it. */
function PathBar({ choice, trail, icon, onGo }: { choice: FolderChoice; trail: number[]; icon: string | null; onGo: (depth: number) => void }) {
  // A long way down shows the first folder, a gap, and the last few, as there's room for.
  const LAST = 4;
  const steps = [0, ...trail];
  const cut = steps.length > LAST + 2 ? steps.length - LAST : 1;
  const shown = steps.map((node, depth) => ({ node, depth })).filter(({ depth }) => depth === 0 || depth >= cut);
  return (
    <ol className="fsc-path-list">
      {shown.map(({ node, depth }, i) => (
        <li key={node} className={depth === steps.length - 1 ? "fsc-step is-here" : "fsc-step"}>
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
            data-tip={choice.name(node)}
            data-tip-overflow
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => onGo(depth)}
          >
            <Icon src={icon} className="fsc-step-icon" />
            <span className="fsc-step-name">{choice.name(node)}</span>
          </button>
        </li>
      ))}
    </ol>
  );
}
