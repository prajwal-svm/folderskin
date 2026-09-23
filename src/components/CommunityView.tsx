import { type CSSProperties, memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type CommunityPack, type CommunitySort, type PackProgress, type Skin, type SkinHit } from "../lib/tauri";
import { isTauri } from "../lib/devMock";
import { licenseLabel, PACKS_GUIDE_URL } from "../lib/packs";
import { tagLabel } from "../lib/tags";
import { community, progressLabel, progressShare, useCommunity, type PackTask, type PackView, type Shown } from "../lib/communityStore";
import type { ToastTone } from "../hooks/useToasts";
import { Confirm } from "./Confirm";
import { GalleryToolbar, type TabCount } from "./GalleryToolbar";
import { OkBadge } from "./OkBadge";
import { PackPreview } from "./PackPreview";
import { PackViewer } from "./PackViewer";
import { VirtualGrid, type VirtualGridHandle } from "./VirtualGrid";
import { DeleteIcon } from "./icons/delete";
import { DownloadIcon } from "./icons/download";
import { EyeIcon } from "./icons/eye";
import { ExternalLinkIcon } from "./icons/external-link";
import { FolderOpenIcon } from "./icons/folder-open";
import { LayoutGridIcon } from "./icons/layout-grid";
import { ListIcon } from "./icons/list";
import { ListFilterIcon } from "./icons/list-filter";
import { LoaderIcon } from "./icons/loader";
import { RefreshCwIcon } from "./icons/refresh-cw";
import { SparklesIcon } from "./icons/sparkles";
import { clip } from "../lib/names";

type Toast = (text: string, opts?: { tone?: ToastTone; action?: { label: string; run: () => void } }) => void;

/** Tags beside "All"; the rest are a click away under the options button. */
const TOP_TAGS = 8;
const GAP = 10;
const PADDING = 14;

/** A card's height for its width: the folders two by two (never over 300 px), then two lines of
 *  name, up to two of byline, a row of tags and the buttons. */
const galleryRow = (width: number) => Math.min(width - 24, 300) + 200;
/** A row's height: the strip beside the words, or, in a narrow window, above them. */
const listRow = (width: number) => (width < 560 ? 240 : 116);

const numbers = new Intl.NumberFormat("en-GB");

/**
 * Skin packs other people shared, searched as you type: by name, author, tag or the name of any
 * skin in them. The list can run to tens of thousands, so only the cards on screen are drawn and
 * only their previews are fetched, a page at a time. What the view shows lives in
 * lib/communityStore, so leaving and coming back finds it as it was.
 */
export function CommunityView({
  onShare,
  onAdded,
  onRemoved,
  onShowTag,
  toast,
}: {
  /** Opens "Share your skins". */
  onShare: () => void;
  onAdded: (skins: Skin[]) => void;
  onRemoved: (skinIds: string[]) => void;
  /** Opens the library filtered by `tag`. */
  onShowTag: (tag: string) => void;
  toast: Toast;
}) {
  const s = useCommunity();
  // The newest of the app's callbacks, for a pack that finishes adding after the view has gone.
  useLayoutEffect(() => community.bind({ onAdded, onRemoved, onShowTag, toast }));
  useEffect(() => community.start(), []);
  const [removing, setRemoving] = useState<CommunityPack | null>(null);
  const grid = useRef<VirtualGridHandle>(null);
  const shown = s.shown;
  const listed = shown !== null;

  // Back where it was on the way in, at the top for a new answer, and remembered as it scrolls.
  useLayoutEffect(() => {
    const el = grid.current?.element();
    if (el) el.scrollTop = community.get().scrollTop;
  }, [shown?.id, s.view]);
  useEffect(() => {
    const el = grid.current?.element();
    if (!el) return;
    const keep = () => community.keepScroll(el.scrollTop);
    el.addEventListener("scroll", keep, { passive: true });
    return () => el.removeEventListener("scroll", keep);
  }, [listed]);

  const tabs = useMemo<TabCount[]>(() => {
    const facets = shown?.facets ?? [];
    const top = facets.slice(0, TOP_TAGS);
    if (s.tag && !top.some((f) => f.tag === s.tag)) top.push({ tag: s.tag, count: facets.find((f) => f.tag === s.tag)?.count ?? 0 });
    return [{ id: "", label: "All", count: shown?.all ?? 0 }, ...top.map((f) => ({ id: f.tag, label: tagLabel(f.tag), count: f.count }))];
  }, [shown, s.tag]);

  const addFromFolder = async () => {
    const path = isTauri()
      ? await open({ directory: true, multiple: false, title: "Choose a pack folder" }).catch(() => null)
      : "/Users/you/Desktop/my-pack";
    if (typeof path !== "string") return;
    try {
      const skins = await api.importPack(path);
      community.mark(skins[0]?.pack ?? undefined, true);
      onAdded(skins);
      const first = skins[0]?.tags[0];
      toast(`Added ${skins.length} skins from that folder`, {
        tone: "ok",
        action: first ? { label: "Show", run: () => onShowTag(first) } : undefined,
      });
    } catch (e) {
      toast(`Couldn't add that pack: ${errorMessage(e)}`, { tone: "danger" });
    }
  };

  const openHit = (hit: SkinHit) => {
    const pack = shown?.hitPacks.find((p) => p.id === hit.pack);
    if (pack) community.open(pack, hit.index);
  };

  const renderPack = useCallback(
    (pack: CommunityPack | undefined, index: number) =>
      pack ? (
        <PackCard
          pack={pack}
          view={s.view}
          task={s.busy === pack.id ? s.task : null}
          blocked={s.busy !== null && s.busy !== pack.id}
          progress={s.busy === pack.id ? s.progress : null}
          tag={s.tag}
          onRemove={setRemoving}
        />
      ) : (
        <PackPlaceholder index={index} view={s.view} />
      ),
    [s.view, s.busy, s.task, s.progress, s.tag],
  );

  const viewing = s.viewing;

  return (
    <section className="community">
      <header className="community-head">
        <div className="community-intro">
          <h2 className="view-title">Community</h2>
          <p className="view-sub">Free skins and packs people share. Add one and its skins join your library.</p>
        </div>
        <div className="community-actions">
          <button type="button" className="btn btn-secondary" onMouseDown={(e) => e.preventDefault()} onClick={() => void addFromFolder()}>
            <FolderOpenIcon size={15} />
            Add from a folder
          </button>
          <button type="button" className="btn btn-primary" onMouseDown={(e) => e.preventDefault()} onClick={onShare}>
            <SparklesIcon size={15} />
            Share your skins
          </button>
        </div>
      </header>

      <GalleryToolbar
        tabs={tabs}
        active={s.tag}
        onChange={(tag) => community.setTag(tag)}
        query={s.query}
        onQuery={(q) => community.setQuery(q)}
        label="filter packs by tag"
        placeholder="Search packs"
        extra={
          <>
            <CommunityOptions sort={s.sort} typed={s.query.trim() !== ""} facets={shown?.facets ?? []} tag={s.tag} />
            <button
              type="button"
              className="icon-btn"
              title="Look for new and updated packs"
              aria-label="refresh packs"
              aria-busy={s.refreshing}
              disabled={s.refreshing}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => void community.refresh()}
            >
              {s.refreshing ? <LoaderIcon size={16} /> : <RefreshCwIcon size={16} />}
            </button>
            <div className="view-switch" role="radiogroup" aria-label="show packs as">
              {(
                [
                  ["gallery", "Gallery", LayoutGridIcon],
                  ["list", "List", ListIcon],
                ] as const
              ).map(([id, label, Icon]) => (
                <button
                  key={id}
                  type="button"
                  role="radio"
                  aria-checked={s.view === id}
                  aria-label={label}
                  title={label}
                  className={s.view === id ? "view-switch-btn is-active" : "view-switch-btn"}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => community.setView(id)}
                >
                  <Icon size={16} />
                </button>
              ))}
            </div>
          </>
        }
      />

      {shown && shown.q && shown.skins.length > 0 && (
        <section className="skin-hits" aria-label="skins whose names match">
          <p className="skin-hits-title">Skins</p>
          <ul className="skin-hits-list">
            {shown.skins.map((hit) => (
              <li key={`${hit.pack}:${hit.index}`}>
                <button type="button" className="skin-hit" title={`${hit.name}, in ${hit.pack_name}`} onClick={() => openHit(hit)}>
                  {hit.thumbnail ? <img src={hit.thumbnail} alt="" draggable={false} loading="lazy" decoding="async" /> : <span className="skin-hit-blank" />}
                  <span className="skin-hit-name">{hit.name}</span>
                  <span className="skin-hit-pack">{hit.pack_name}</span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}

      <p className="community-status" aria-live="polite">
        <span className="community-count" data-results-for={shown ? shown.q : undefined}>
          {status(shown, s.error)}
          {s.searching && shown && <LoaderIcon size={13} />}
        </span>
        <button type="button" className="link-btn" onClick={() => void openUrl(PACKS_GUIDE_URL).catch(() => {})}>
          Every pack is checked before it's listed
          <ExternalLinkIcon size={13} />
        </button>
      </p>

      {shown === null ? (
        s.error ? (
          <div className="empty">
            <span className="empty-glyph">
              <DownloadIcon size={20} />
            </span>
            <p className="empty-title">The packs didn't load</p>
            <p className="empty-text">{s.error.charAt(0).toUpperCase() + s.error.slice(1)}. You can still add a pack from a folder.</p>
            <button type="button" className="btn btn-secondary" onClick={() => void community.search()}>
              Try again
            </button>
          </div>
        ) : (
          <p className="community-note">
            <LoaderIcon /> Loading packs
          </p>
        )
      ) : (
        <VirtualGrid
          ref={grid}
          className={s.view === "gallery" ? "packs-grid is-gallery" : "packs-grid is-list"}
          items={shown.packs}
          minCell={s.view === "gallery" ? 250 : 100_000}
          gap={GAP}
          padding={PADDING}
          rowHeight={s.view === "gallery" ? galleryRow : listRow}
          getKey={(pack, i) => pack?.id ?? `place-${i}`}
          renderItem={renderPack}
          role="region"
          aria-label="community packs"
          empty={
            <div className="empty">
              <p className="empty-title">{shown.all || shown.q || shown.tag ? "No pack matches" : "No packs yet"}</p>
              <p className="empty-text">{shown.all || shown.q || shown.tag ? "Try another word or tag." : "Be the first: share yours."}</p>
            </div>
          }
        />
      )}

      {viewing && (
        <PackViewer
          key={`${viewing.pack.id}:${viewing.focus ?? ""}`}
          pack={viewing.pack}
          focus={viewing.focus}
          busy={s.busy === viewing.pack.id}
          removing={s.busy === viewing.pack.id && s.task === "remove"}
          progress={s.busy === viewing.pack.id ? s.progress : null}
          blocked={s.busy !== null && s.busy !== viewing.pack.id}
          onAdd={() => void community.add(viewing.pack)}
          onUpdate={() => void community.update(viewing.pack)}
          onRemove={() => setRemoving(viewing.pack)}
          onClose={() => community.close()}
        />
      )}

      {removing && (
        <Confirm
          title={`Remove "${clip(removing.name)}"?`}
          text={`Its skins leave your library. Folders that already use them keep their icon.`}
          action="Remove"
          onCancel={() => setRemoving(null)}
          onConfirm={() => {
            setRemoving(null);
            void community.remove(removing);
          }}
        />
      )}
    </section>
  );
}

/** What the list holds, in a few words: "10,000 packs", "23 packs match “koi”". */
function status(shown: Shown | null, error: string | null): string {
  if (!shown) return "";
  if (error) return `Couldn't search: ${error}`;
  const packs = `${numbers.format(shown.total)} ${shown.total === 1 ? "pack" : "packs"}`;
  const words = shown.q ? ` match “${shown.q}”` : "";
  const tagged = shown.tag ? ` tagged ${tagLabel(shown.tag)}` : "";
  const offline = shown.offline ? ". You're offline, so these are the packs from your last visit" : "";
  return `${packs}${words}${tagged}${offline}`;
}

/** One pack: its folders, who made it and its tags, and Add (or Update and Remove). */
const PackCard = memo(function PackCard({
  pack,
  view,
  task,
  blocked,
  progress,
  tag,
  onRemove,
}: {
  pack: CommunityPack;
  view: PackView;
  /** What is being done to this pack, if anything. */
  task: PackTask | null;
  /** Something is being done to another pack. */
  blocked: boolean;
  progress: PackProgress | null;
  /** The tag the list is filtered by. */
  tag: string;
  onRemove: (pack: CommunityPack) => void;
}) {
  const gallery = view === "gallery";
  const busy = task !== null;
  return (
    <article className="pack" aria-label={pack.name}>
      <button
        type="button"
        className="pack-preview-btn"
        aria-label={`view ${pack.name}`}
        title={`View ${pack.name}`}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => community.open(pack)}
      >
        <PackPreview src={pack.preview} count={pack.count} grid={gallery} />
      </button>
      <div className="pack-meta">
        <div className="pack-title">
          <p className="pack-name">{pack.name}</p>
          {pack.added && <OkBadge size={20} label="Added to your library" />}
        </div>
        <p className="pack-by">
          by{" "}
          <button type="button" className="pack-author" onClick={() => void openUrl(`https://github.com/${pack.author}`).catch(() => {})}>
            @{pack.author}
          </button>{" "}
          · {pack.count} {pack.count === 1 ? "skin" : "skins"} · {licenseLabel(pack.license)}
        </p>
        <div className="pack-tags">
          {pack.tags.slice(0, 4).map((t) => (
            <button
              type="button"
              key={t}
              className={t === tag ? "tag-chip is-link is-active" : "tag-chip is-link"}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => community.setTag(t === tag ? "" : t)}
            >
              {t}
            </button>
          ))}
        </div>
      </div>
      <div className="pack-action">
        {task === "add" || task === "update" ? (
          <PackWorking name={pack.name} task={task} progress={progress} />
        ) : (
          <>
            <button type="button" className="btn btn-secondary" onMouseDown={(e) => e.preventDefault()} onClick={() => community.open(pack)}>
              <EyeIcon size={15} />
              View
            </button>
            {pack.added ? (
              <>
                {pack.update && (
                  <button
                    type="button"
                    className="btn btn-primary"
                    title="A newer version of this pack is out"
                    disabled={busy || blocked}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => void community.update(pack)}
                  >
                    <RefreshCwIcon size={15} />
                    Update
                  </button>
                )}
                {!gallery ? (
                  <button type="button" className="btn btn-ghost" disabled={busy} aria-busy={busy} onMouseDown={(e) => e.preventDefault()} onClick={() => onRemove(pack)}>
                    {busy ? "Removing" : "Remove"}
                  </button>
                ) : busy ? (
                  <span className="icon-btn pack-remove" role="status" aria-label={`removing ${pack.name}`}>
                    <LoaderIcon size={16} />
                  </span>
                ) : (
                  // A card is narrow: the can alone, round like the buttons beside it.
                  <button
                    type="button"
                    className="icon-btn pack-remove"
                    title="Remove this pack"
                    aria-label={`remove ${pack.name}`}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => onRemove(pack)}
                  >
                    <DeleteIcon size={16} />
                  </button>
                )}
              </>
            ) : (
              <button type="button" className="btn btn-primary" disabled={blocked} onMouseDown={(e) => e.preventDefault()} onClick={() => void community.add(pack)}>
                <DownloadIcon size={15} />
                Add
              </button>
            )}
          </>
        )}
      </div>
    </article>
  );
});

/** How far adding or updating a pack has got, in place of its buttons. */
function PackWorking({ name, task, progress }: { name: string; task: "add" | "update"; progress: PackProgress | null }) {
  const share = progressShare(progress);
  const verb = task === "add" ? "Adding" : "Updating";
  const label = progressLabel(progress, verb);
  return (
    <div
      className="pack-progress"
      role="progressbar"
      aria-label={`${verb} ${name}`}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(share * 100)}
      aria-valuetext={label}
      style={{ "--done": share } as CSSProperties}
    >
      <span className="pack-progress-text">
        <LoaderIcon size={14} />
        {label}
      </span>
    </div>
  );
}

/** A place in the list whose page hasn't come yet: asks for it, and shimmers until it's here. */
function PackPlaceholder({ index, view }: { index: number; view: PackView }) {
  useEffect(() => community.need(index), [index]);
  return (
    <div className="pack is-placeholder" aria-hidden="true">
      <span className="pack-preview">
        <span className="pack-preview-blank" />
      </span>
      {view === "gallery" && (
        <span className="pack-meta">
          <span className="pack-line" />
          <span className="pack-line is-short" />
        </span>
      )}
    </div>
  );
}

const SORTS: { id: CommunitySort; label: (typed: boolean) => string }[] = [
  { id: "best", label: (typed) => (typed ? "Best match" : "Featured") },
  { id: "newest", label: () => "Newest" },
  { id: "name", label: () => "Name" },
  { id: "skins", label: () => "Most skins" },
];

const WIDTH = 320;
const MARGIN = 12;

/** The button beside the search that opens the order of the list and every tag. */
function CommunityOptions({ sort, typed, facets, tag }: { sort: CommunitySort; typed: boolean; facets: { tag: string; count: number }[]; tag: string }) {
  const [openNow, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const close = useCallback(() => setOpen(false), []);
  const hiddenTag = tag !== "" && facets.findIndex((f) => f.tag === tag) >= TOP_TAGS;
  return (
    <>
      <button
        ref={button}
        type="button"
        className={sort !== "best" || hiddenTag ? "filter-btn is-on" : "filter-btn"}
        aria-haspopup="dialog"
        aria-expanded={openNow}
        aria-label="Sort and more tags"
        title="Sort and more tags"
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => setOpen((o) => !o)}
      >
        <ListFilterIcon size={16} />
      </button>
      {openNow && button.current && <OptionsPopover anchor={button.current} sort={sort} typed={typed} facets={facets} tag={tag} onClose={close} />}
    </>
  );
}

function OptionsPopover({
  anchor,
  sort,
  typed,
  facets,
  tag,
  onClose,
}: {
  anchor: HTMLElement;
  sort: CommunitySort;
  typed: boolean;
  facets: { tag: string; count: number }[];
  tag: string;
  onClose: () => void;
}) {
  const panel = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number; maxHeight: number } | null>(null);

  // Under the button, its right edge lined up with the button's, inside the window.
  useLayoutEffect(() => {
    const place = () => {
      const a = anchor.getBoundingClientRect();
      const left = Math.min(Math.max(MARGIN, a.right - WIDTH), window.innerWidth - WIDTH - MARGIN);
      const top = a.bottom + 8;
      setPos({ left, top, maxHeight: Math.max(160, window.innerHeight - top - MARGIN) });
    };
    place();
    window.addEventListener("resize", place);
    return () => window.removeEventListener("resize", place);
  }, [anchor]);

  useEffect(() => panel.current?.focus({ preventScroll: true }), []);

  useEffect(() => {
    const down = (e: MouseEvent) => {
      const target = e.target as Node;
      if (!panel.current?.contains(target) && !anchor.contains(target)) onClose();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      onClose();
      anchor.focus({ preventScroll: true });
    };
    document.addEventListener("mousedown", down);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("mousedown", down);
      document.removeEventListener("keydown", key);
    };
  }, [anchor, onClose]);

  return createPortal(
    <div
      ref={panel}
      className="filter-pop"
      role="dialog"
      aria-label="sort and tags"
      tabIndex={-1}
      style={pos ? { left: pos.left, top: pos.top, width: WIDTH, maxHeight: pos.maxHeight } : { visibility: "hidden" }}
    >
      <div className="filter-head">
        <div>
          <p className="filter-title">Sort and tags</p>
          <p className="filter-sub">{facets.length === 1 ? "1 tag" : `${numbers.format(facets.length)} tags`} in these packs</p>
        </div>
        {tag && (
          <button type="button" className="filter-clear" onClick={() => community.setTag("")}>
            All tags
          </button>
        )}
      </div>

      <section className="filter-sec">
        <p className="filter-label">Sort by</p>
        <div className="seg seg-sm filter-sort community-sort" role="radiogroup" aria-label="sort by">
          {SORTS.map((s) => (
            <button
              key={s.id}
              type="button"
              role="radio"
              aria-checked={sort === s.id}
              className={sort === s.id ? "seg-btn is-active" : "seg-btn"}
              onClick={() => community.setSort(s.id)}
            >
              {s.label(typed)}
            </button>
          ))}
        </div>
      </section>

      {facets.length > 0 && (
        <section className="filter-sec">
          <p className="filter-label">Tags</p>
          <div className="filter-chips">
            {facets.map((f) => {
              const on = f.tag === tag;
              return (
                <button
                  key={f.tag}
                  type="button"
                  aria-pressed={on}
                  className={on ? "fchip is-on" : "fchip"}
                  onClick={() => community.setTag(on ? "" : f.tag)}
                >
                  <span className="fchip-label">{tagLabel(f.tag)}</span>
                  <span className="count">{numbers.format(f.count)}</span>
                </button>
              );
            })}
          </div>
        </section>
      )}
    </div>,
    document.body,
  );
}
