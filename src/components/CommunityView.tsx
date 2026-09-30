import { type CSSProperties, memo, type ReactNode, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type CollectionSkin, type CollectionSort, type CommunityPack, type CommunitySort, type PackProgress, type Skin, type SkinHit } from "../lib/tauri";
import { isTauri } from "../lib/devMock";
import { licenseLabel } from "../lib/packs";
import { docsUrl, t as tNow, useT } from "../i18n";
import { formatNumber } from "../i18n/format";
import { Rich } from "../i18n/Rich";
import { tagLabel } from "../lib/tags";
import { collectionCountLine, community, countLine, progressLabel, progressShare, useCommunity, type PackTask, type PackView } from "../lib/communityStore";
import { inLibrary, libraryIndex, packInLibrary } from "../lib/inLibrary";
import type { ToastTone } from "../hooks/useToasts";
import { Confirm } from "./Confirm";
import { GalleryToolbar, type TabCount } from "./GalleryToolbar";
import { HaveChip, InfoTip } from "./InLibrary";
import { DrivesBadge, OfficialBadge } from "./OfficialBadge";
import { OkBadge } from "./OkBadge";
import { PackPreview } from "./PackPreview";
import { PackViewer } from "./PackViewer";
import { SkinViewer } from "./SkinViewer";
import { VirtualGrid, type VirtualGridHandle } from "./VirtualGrid";
import { BadgeCheckIcon } from "./icons/badge-check";
import { DeleteIcon } from "./icons/delete";
import { DownloadIcon } from "./icons/download";
import { EyeIcon } from "./icons/eye";
import { FolderOpenIcon } from "./icons/folder-open";
import { InfoIcon } from "./icons/info";
import { LayoutGridIcon } from "./icons/layout-grid";
import { ListIcon } from "./icons/list";
import { ListFilterIcon } from "./icons/list-filter";
import { LoaderIcon } from "./icons/loader";
import { RefreshCwIcon } from "./icons/refresh-cw";
import { SearchIcon } from "./icons/search";
import { SparklesIcon } from "./icons/sparkles";
import { clip } from "../lib/names";

type Toast = (text: string, opts?: { tone?: ToastTone; action?: { label: string; run: () => void } }) => void;

/** Tags beside "All"; the rest are a click away under the options button. */
const TOP_TAGS = 8;
/** The Official tab's id among the tags', which no tag can be: a tag has no colon (lib/tags.ts). */
const OFFICIAL_TAB = ":official";
const GAP = 10;
const PADDING = 14;

/** A card's height for its width: the folders two by two (never over 300 px), then two lines of
 *  name, up to two of byline, a row of tags and the buttons. */
const galleryRow = (width: number) => Math.min(width - 24, 300) + 200;
/** A row's height: the strip beside the words, or, in a narrow window, above them. */
const listRow = (width: number) => (width < 560 ? 240 : 116);
/** An official skin's height for its width: the folder, square, then a line of name. */
const skinRow = (width: number) => width + 28;


/**
 * Skin packs other people shared, searched as you type: by name, author, tag or the name of any
 * skin in them. The list can run to tens of thousands, so only the cards on screen are drawn and
 * only their previews are fetched, a page at a time. What the view shows lives in
 * lib/communityStore, so leaving and coming back finds it as it was.
 *
 * The Official tab, first in the row, lists FolderSkin's own skins instead: not packs but skins,
 * searched and paged the same way, each opened in a skin viewer and used one at a time.
 *
 * Skins in the library already wear a tick, wherever they show: on the official skins, in the
 * strip of skins, in a pack opened to look through, and, counted, on a pack's card while the pack
 * itself isn't added. The ticks follow `library` as it changes (lib/inLibrary.ts).
 */
export function CommunityView({
  library,
  onShare,
  onAdded,
  onRemoved,
  onUsed,
  onShowSkin,
  onShowTag,
  toast,
}: {
  /** The skins in the library. */
  library: Skin[];
  /** Opens "Share your skins". */
  onShare: () => void;
  onAdded: (skins: Skin[]) => void;
  onRemoved: (skinIds: string[]) => void;
  /** Picks a skin taken on its own ("Use"), now in the library. */
  onUsed: (skin: Skin) => void;
  /** Picks skin `id` of the library's, where Apply is. */
  onShowSkin: (id: string) => void;
  /** Opens the library filtered by `tag`. */
  onShowTag: (tag: string) => void;
  toast: Toast;
}) {
  const t = useT();
  const s = useCommunity();
  const kept = useMemo(() => libraryIndex(library), [library]);
  // The newest of the app's callbacks, for a pack that finishes adding after the view has gone.
  useLayoutEffect(() => community.bind({ onAdded, onRemoved, onUsed, onShowTag, toast }));
  useEffect(() => community.start(), []);
  const [removing, setRemoving] = useState<CommunityPack | null>(null);
  const grid = useRef<VirtualGridHandle>(null);
  const shown = s.shown;
  const official = s.tab === "official";
  const collection = s.collection;
  const listed = official ? collection !== null : shown !== null;

  // Back where it was on the way in, at the top for a new answer, and remembered as it scrolls.
  // Each tab keeps its own place.
  useLayoutEffect(() => {
    const el = grid.current?.element();
    const now = community.get();
    if (el) el.scrollTop = now.tab === "official" ? now.collectionScroll : now.scrollTop;
  }, [shown?.id, s.view, s.tab, collection?.id]);
  useEffect(() => {
    const el = grid.current?.element();
    if (!el) return;
    const keep = () => community.keepScroll(el.scrollTop);
    el.addEventListener("scroll", keep, { passive: true });
    return () => el.removeEventListener("scroll", keep);
  }, [listed, s.tab]);

  // Official comes first, once there are official skins (or while it's the tab on show), in the
  // Official badge's colours.
  const officialCount = official ? (collection?.total ?? s.collectionSize ?? 0) : (s.collectionSize ?? 0);
  const tabs = useMemo<TabCount[]>(() => {
    const facets = shown?.facets ?? [];
    const top = facets.slice(0, TOP_TAGS);
    if (s.tag && !top.some((f) => f.tag === s.tag)) top.push({ tag: s.tag, count: facets.find((f) => f.tag === s.tag)?.count ?? 0 });
    const first: TabCount[] =
      official || officialCount > 0
        ? [{ id: OFFICIAL_TAB, label: t("common.official"), count: officialCount, icon: <BadgeCheckIcon size={14} />, className: "is-official" }]
        : [];
    return [
      ...first,
      { id: "", label: t("library.tabs.all"), count: shown?.all ?? 0 },
      ...top.map((f) => ({ id: f.tag, label: tagLabel(f.tag), count: f.count })),
    ];
  }, [shown, s.tag, official, officialCount, t]);
  // The search box says where it looks: in the tag picked, so nobody takes it for a search of every pack.
  const tagName = s.tag ? (tabs.find((tab) => tab.id === s.tag)?.label ?? tagLabel(s.tag)) : "";

  const addFromFolder = async () => {
    const path = isTauri()
      ? await open({ directory: true, multiple: false, title: tNow("community.dialog.choosePackFolder") }).catch(() => null)
      : "/Users/you/Desktop/my-pack";
    if (typeof path !== "string") return;
    try {
      const skins = await api.importPack(path);
      community.mark(skins[0]?.pack ?? undefined, true);
      onAdded(skins);
      const first = skins[0]?.tags[0];
      toast(tNow("community.toast.addedFromFolder", { count: skins.length }), {
        tone: "ok",
        action: first ? { label: tNow("community.toast.show"), run: () => onShowTag(first) } : undefined,
      });
    } catch (e) {
      toast(tNow("community.toast.addFolderFailed", { reason: errorMessage(e) }), { tone: "danger" });
    }
  };

  const openHit = (hit: SkinHit) => {
    const pack = shown?.hitPacks.find((p) => p.id === hit.pack);
    if (pack) community.open(pack, hit.index);
  };
  // Official skins lead the strip of skins, in All: a tag narrows packs, and they're in none.
  const officialHits = shown && !shown.tag ? shown.official : [];

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
          have={packInLibrary(kept, pack)}
          onRemove={setRemoving}
        />
      ) : (
        <PackPlaceholder index={index} view={s.view} />
      ),
    [s.view, s.busy, s.task, s.progress, s.tag, kept],
  );

  const renderSkin = useCallback(
    (skin: CollectionSkin | undefined, index: number) =>
      skin ? <OfficialTile skin={skin} kept={inLibrary(kept, skin.skin_id)} /> : <SkinPlaceholder index={index} />,
    [kept],
  );

  const viewing = s.viewing;
  const viewingSkin = s.viewingSkin;

  return (
    <section className="community">
      <header className="community-head" data-tauri-drag-region>
        <div className="community-intro">
          <h2 className="view-title">{t("community.title")}</h2>
          <p className="view-sub">{t("community.sub")}</p>
        </div>
        <div className="community-actions">
          <button type="button" className="btn btn-secondary" onMouseDown={(e) => e.preventDefault()} onClick={() => void addFromFolder()}>
            <FolderOpenIcon size={15} />
            <span className="btn-label">{t("community.addFromFolder")}</span>
          </button>
          <button type="button" className="btn btn-primary" onMouseDown={(e) => e.preventDefault()} onClick={onShare}>
            <SparklesIcon size={15} />
            <span className="btn-label">{t("community.shareYours")}</span>
          </button>
        </div>
      </header>

      <GalleryToolbar
        tabs={tabs}
        active={official ? OFFICIAL_TAB : s.tag}
        onChange={(id) => (id === OFFICIAL_TAB ? community.showOfficial() : community.setTag(id))}
        query={s.query}
        onQuery={(q) => community.setQuery(q)}
        label={t("community.tabsLabel")}
        placeholder={official ? t("community.official.search") : tagName ? t("community.searchIn", { tag: tagName }) : t("community.search")}
        searchLabel={official ? t("community.official.searchLabel") : tagName ? t("community.searchInLabel", { tag: tagName }) : t("community.searchLabel")}
        extra={
          <>
            {official ? (
              <OfficialOptions sort={s.collectionSort} />
            ) : (
              <CommunityOptions sort={s.sort} typed={s.query.trim() !== ""} facets={shown?.facets ?? []} tag={s.tag} />
            )}
            <button
              type="button"
              className="icon-btn"
              data-tip={t("community.refreshTip")}
              aria-label={t("community.refreshLabel")}
              aria-busy={s.refreshing}
              disabled={s.refreshing}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => void community.refresh()}
            >
              {s.refreshing ? <LoaderIcon size={16} /> : <RefreshCwIcon size={16} />}
            </button>
            {/* Gallery or list is for packs: an official skin is one folder, whichever. */}
            {!official && (
              <div className="view-switch" role="radiogroup" aria-label={t("community.viewLabel")}>
                {(
                  [
                    ["gallery", t("community.views.gallery"), LayoutGridIcon],
                    ["list", t("community.views.list"), ListIcon],
                  ] as const
                ).map(([id, label, Icon]) => (
                  <button
                    key={id}
                    type="button"
                    role="radio"
                    aria-checked={s.view === id}
                    aria-label={label}
                    data-tip={label}
                    className={s.view === id ? "view-switch-btn is-active" : "view-switch-btn"}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => community.setView(id)}
                  >
                    <Icon size={16} />
                  </button>
                ))}
              </div>
            )}
          </>
        }
      />

      {!official && shown && shown.q && shown.skins.length + officialHits.length > 0 && (
        <section className="skin-hits" aria-label={t("community.hits.label")}>
          <p className="skin-hits-title">{t("community.hits.title")}</p>
          <ul className="skin-hits-list">
            {officialHits.map((skin) => (
              <li key={skin.sha256}>
                <button type="button" className="skin-hit is-official" data-tip={skin.name} data-tip-overflow onClick={() => community.openSkin(skin)}>
                  <span className="skin-hit-art">
                    <img src={skin.thumbnail} alt="" draggable={false} loading="lazy" decoding="async" />
                    {inLibrary(kept, skin.skin_id) && <OkBadge size={16} label={t("community.inLibrary.label")} />}
                  </span>
                  <span className="skin-hit-name">{skin.name}</span>
                  <span className="skin-hit-mark">
                    <OfficialBadge />
                  </span>
                </button>
              </li>
            ))}
            {shown.skins.map((hit) => (
              <li key={`${hit.pack}:${hit.index}`}>
                <button type="button" className="skin-hit" data-tip={t("community.hits.tip", { name: hit.name, pack: hit.pack_name })} data-tip-overflow onClick={() => openHit(hit)}>
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
        {official ? (
          <span className="community-count" data-results-for={collection ? collection.q : undefined}>
            {collectionCountLine(collection, s.collectionError)}
            {s.collectionSearching && collection && <LoaderIcon size={13} />}
          </span>
        ) : (
          <span className="community-count" data-results-for={shown ? shown.q : undefined}>
            {countLine(shown, s.error)}
            {s.searching && shown && <LoaderIcon size={13} />}
            {shown?.lastVisit && !s.error && (
              <button type="button" className="link-btn" disabled={s.refreshing} onClick={() => void community.refresh()}>
                {t("community.tryAgain")}
              </button>
            )}
          </span>
        )}
        {/* The official skins aren't packs, so their ⓘ says what the tick and Use mean instead. */}
        {official ? (
          <InfoTip className="icon-btn community-checked" text={t("community.inLibrary.officialInfo")} />
        ) : (
          <button
            type="button"
            className="icon-btn community-checked"
            data-tip={t("community.checked")}
            aria-label={t("community.checkedLabel")}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => void openUrl(docsUrl("packs")).catch(() => {})}
          >
            <InfoIcon size={15} />
          </button>
        )}
      </p>

      {official ? (
        collection === null ? (
          s.collectionError ? (
            <div className="empty">
              <span className="empty-glyph">
                <DownloadIcon size={20} />
              </span>
              <p className="empty-title">{t("community.official.loadFailed")}</p>
              <p className="empty-text">{t("community.official.loadFailedText", { reason: s.collectionError.charAt(0).toUpperCase() + s.collectionError.slice(1) })}</p>
              <button type="button" className="btn btn-secondary" onClick={() => void community.searchCollection()}>
                {t("community.tryAgain")}
              </button>
            </div>
          ) : (
            <p className="community-note">
              <LoaderIcon /> {t("community.official.loading")}
            </p>
          )
        ) : (
          <VirtualGrid
            key="official"
            ref={grid}
            className="packs-grid is-skins"
            items={collection.skins}
            minCell={136}
            gap={GAP}
            padding={PADDING}
            rowHeight={skinRow}
            getKey={(skin, i) => skin?.sha256 ?? `place-${i}`}
            renderItem={renderSkin}
            role="region"
            aria-label={t("community.official.gridLabel")}
            empty={
              <div className="empty">
                <p className="empty-title">{collection.q ? t("community.official.empty.noMatch") : t("community.official.empty.none")}</p>
                <p className="empty-text">{collection.q ? t("community.official.empty.noMatchText") : t("community.official.empty.noneText")}</p>
                {/* The same words may well find packs. */}
                {collection.q && (
                  <button type="button" className="btn btn-secondary" onClick={() => community.setTag("")}>
                    <SearchIcon size={15} />
                    {t("community.official.empty.searchPacks")}
                  </button>
                )}
              </div>
            }
          />
        )
      ) : shown === null ? (
        s.error ? (
          <div className="empty">
            <span className="empty-glyph">
              <DownloadIcon size={20} />
            </span>
            <p className="empty-title">{t("community.loadFailed")}</p>
            <p className="empty-text">{t("community.loadFailedText", { reason: s.error.charAt(0).toUpperCase() + s.error.slice(1) })}</p>
            <button type="button" className="btn btn-secondary" onClick={() => void community.search()}>
              {t("community.tryAgain")}
            </button>
          </div>
        ) : (
          <p className="community-note">
            <LoaderIcon /> {t("community.loading")}
          </p>
        )
      ) : (
        <VirtualGrid
          key="packs"
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
          aria-label={t("community.gridLabel")}
          empty={
            <div className="empty">
              <p className="empty-title">{shown.all || shown.q || shown.tag ? t("community.empty.noMatch") : t("community.empty.none")}</p>
              <p className="empty-text">{shown.all || shown.q || shown.tag ? t("community.empty.noMatchText") : t("community.empty.noneText")}</p>
              {/* Nothing in the tag picked: the same words may well be in another. */}
              {shown.q && shown.tag && (
                <button type="button" className="btn btn-secondary" onClick={() => community.setTag("")}>
                  <SearchIcon size={15} />
                  {t("community.empty.searchAll")}
                </button>
              )}
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
          using={s.using}
          onUse={(skin) => skin.sha256 && void community.use({ kind: "pack", id: viewing.pack.id, hash: viewing.pack.hash }, skin.sha256, skin.name)}
          library={kept}
        />
      )}

      {viewingSkin && (
        <SkinViewer
          key={viewingSkin.sha256}
          skin={viewingSkin}
          license={s.collectionLicense}
          using={s.using === viewingSkin.sha256}
          blocked={s.using !== null && s.using !== viewingSkin.sha256}
          kept={inLibrary(kept, viewingSkin.skin_id)}
          onUse={() => void community.use({ kind: "collection" }, viewingSkin.sha256, viewingSkin.name)}
          onShow={() => {
            community.closeSkin();
            if (viewingSkin.skin_id) onShowSkin(viewingSkin.skin_id);
          }}
          onClose={() => community.closeSkin()}
        />
      )}

      {removing && (
        <Confirm
          title={t("community.remove.title", { name: clip(removing.name) })}
          text={t("community.remove.text")}
          action={t("community.remove.action")}
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

/** One pack: its folders, who made it and its tags, and Add (or Update and Remove). A pack not
 *  added with some of its skins in the library says how many beside its name. */
const PackCard = memo(function PackCard({
  pack,
  view,
  task,
  blocked,
  progress,
  tag,
  have,
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
  /** How many of its skins are in the library while it isn't. */
  have: number;
  onRemove: (pack: CommunityPack) => void;
}) {
  const t = useT();
  const gallery = view === "gallery";
  const busy = task !== null;
  return (
    <article className="pack" aria-label={pack.name}>
      <button
        type="button"
        className="pack-preview-btn"
        aria-label={t("community.pack.viewLabel", { name: pack.name })}
        data-tip={t("community.pack.viewTip", { name: pack.name })}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => community.open(pack)}
      >
        <PackPreview src={pack.preview} count={pack.count} grid={gallery} />
      </button>
      <div className="pack-meta">
        <div className="pack-title">
          <p className="pack-name">{pack.name}</p>
          {pack.added ? <OkBadge size={20} label={t("community.pack.added")} /> : have > 0 && <HaveChip have={have} total={pack.count} />}
        </div>
        <p className="pack-by">
          <Rich k="community.pack.by" vars={{ author: pack.author }} tags={{ a: (s) => <span className="pack-author">{s}</span> }} /> ·{" "}
          {t("community.pack.skins", { count: pack.count })} · {licenseLabel(pack.license)}
        </p>
        <div className="pack-tags">
          {pack.official && <OfficialBadge />}
          {pack.drives && <DrivesBadge />}
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
            {/* Update comes first, so View always sits beside the last button, whatever else a pack offers. */}
            {pack.added && pack.update && (
              <button
                type="button"
                className="btn btn-primary"
                data-tip={t("community.pack.updateTip")}
                disabled={busy || blocked}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => void community.update(pack)}
              >
                <RefreshCwIcon size={15} />
                {t("community.pack.update")}
              </button>
            )}
            <button type="button" className="btn btn-secondary" onMouseDown={(e) => e.preventDefault()} onClick={() => community.open(pack)}>
              <EyeIcon size={15} />
              {t("community.pack.view")}
            </button>
            {pack.added ? (
              <>
                {!gallery ? (
                  <button
                    type="button"
                    className="btn btn-ghost"
                    disabled={busy || blocked}
                    aria-busy={busy}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => onRemove(pack)}
                  >
                    {busy ? t("community.pack.removing") : t("community.remove.action")}
                  </button>
                ) : busy ? (
                  <span className="icon-btn pack-remove" role="status" aria-label={t("community.pack.removingLabel", { name: pack.name })}>
                    <LoaderIcon size={16} />
                  </span>
                ) : (
                  // A card is narrow: the can alone, round like the buttons beside it.
                  <button
                    type="button"
                    className="icon-btn pack-remove"
                    data-tip={t("community.pack.removeTip")}
                    aria-label={t("community.pack.removeLabel", { name: pack.name })}
                    disabled={blocked}
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
                {t("community.pack.add")}
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
  const t = useT();
  const share = progressShare(progress);
  const verb = task === "add" ? t("community.progress.adding") : t("community.progress.updating");
  const label = progressLabel(progress, verb);
  return (
    <div
      className="pack-progress"
      role="progressbar"
      aria-label={task === "add" ? t("community.progress.addingLabel", { name }) : t("community.progress.updatingLabel", { name })}
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

/** One official skin: the folder it makes and its name, which open it in the skin viewer, and a
 *  tick in the corner when it's in the library already. */
const OfficialTile = memo(function OfficialTile({ skin, kept }: { skin: CollectionSkin; kept: boolean }) {
  const t = useT();
  return (
    <div className="tile">
      <button type="button" className="tile-hit" onMouseDown={(e) => e.preventDefault()} onClick={() => community.openSkin(skin)}>
        <span className="tile-art">
          <img className="tile-img" src={skin.thumbnail} alt="" draggable={false} loading="lazy" decoding="async" />
          {kept && (
            <span className="tile-kept">
              <OkBadge size={20} label={t("community.inLibrary.label")} />
            </span>
          )}
        </span>
        <span className="tile-name" data-tip={skin.name} data-tip-overflow>
          <span className="tile-name-text">{skin.name}</span>
        </span>
      </button>
    </div>
  );
});

/** A place among the official skins whose page hasn't come yet: asks for it, and shimmers until it's here. */
function SkinPlaceholder({ index }: { index: number }) {
  useEffect(() => community.needSkin(index), [index]);
  return (
    <div className="tile is-placeholder" aria-hidden="true">
      <span className="tile-hit">
        <span className="tile-art">
          <span className="skin-blank" />
        </span>
        <span className="pack-line is-short" />
      </span>
    </div>
  );
}

const SORTS: { id: CommunitySort; label: (typed: boolean) => string }[] = [
  { id: "best", label: (typed) => (typed ? tNow("community.sort.bestMatch") : tNow("community.sort.featured")) },
  { id: "newest", label: () => tNow("community.sort.newest") },
  { id: "name", label: () => tNow("community.sort.name") },
  { id: "skins", label: () => tNow("community.sort.mostSkins") },
];

const WIDTH = 320;
const MARGIN = 12;

const OFFICIAL_SORTS: { id: CollectionSort; label: () => string }[] = [
  { id: "newest", label: () => tNow("community.sort.newest") },
  { id: "name", label: () => tNow("community.sort.name") },
];

/** The button beside the search that orders the official skins: they have no tags to pick. */
function OfficialOptions({ sort }: { sort: CollectionSort }) {
  const t = useT();
  const [openNow, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const close = useCallback(() => setOpen(false), []);
  return (
    <>
      <button
        ref={button}
        type="button"
        className={sort !== "newest" ? "filter-btn is-on" : "filter-btn"}
        aria-haspopup="dialog"
        aria-expanded={openNow}
        aria-label={t("community.official.sortButton")}
        data-tip={t("community.official.sortButton")}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => setOpen((o) => !o)}
      >
        <ListFilterIcon size={16} />
      </button>
      {openNow && button.current && (
        <OptionsPopover anchor={button.current} label={t("community.official.sortLabel")} title={t("community.official.sortButton")} onClose={close}>
          <section className="filter-sec">
            <p className="filter-label">{t("library.filters.sortBy")}</p>
            <div className="seg seg-sm filter-sort community-sort" role="radiogroup" aria-label={t("library.filters.sortByLabel")}>
              {OFFICIAL_SORTS.map((s) => (
                <button
                  key={s.id}
                  type="button"
                  role="radio"
                  aria-checked={sort === s.id}
                  className={sort === s.id ? "seg-btn is-active" : "seg-btn"}
                  onClick={() => community.setCollectionSort(s.id)}
                >
                  {s.label()}
                </button>
              ))}
            </div>
          </section>
        </OptionsPopover>
      )}
    </>
  );
}

/** The button beside the search that opens the order of the list and every tag. */
function CommunityOptions({ sort, typed, facets, tag }: { sort: CommunitySort; typed: boolean; facets: { tag: string; count: number }[]; tag: string }) {
  const t = useT();
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
        aria-label={t("community.options.button")}
        data-tip={t("community.options.button")}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => setOpen((o) => !o)}
      >
        <ListFilterIcon size={16} />
      </button>
      {openNow && button.current && (
        <OptionsPopover
          anchor={button.current}
          label={t("community.options.label")}
          title={t("community.options.title")}
          sub={t("community.options.tagCount", { count: facets.length })}
          action={
            tag && (
              <button type="button" className="filter-clear" onClick={() => community.setTag("")}>
                {t("community.options.allTags")}
              </button>
            )
          }
          onClose={close}
        >
          <section className="filter-sec">
            <p className="filter-label">{t("library.filters.sortBy")}</p>
            <div className="seg seg-sm filter-sort community-sort" role="radiogroup" aria-label={t("library.filters.sortByLabel")}>
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
              <p className="filter-label">{t("community.options.tags")}</p>
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
                      <span className="count">{formatNumber(f.count)}</span>
                    </button>
                  );
                })}
              </div>
            </section>
          )}
        </OptionsPopover>
      )}
    </>
  );
}

/** A popover under an options button: its heading, then whatever it offers. Escape or a click
 *  outside closes it. */
function OptionsPopover({
  anchor,
  label,
  title,
  sub,
  action,
  onClose,
  children,
}: {
  anchor: HTMLElement;
  /** Its name for screen readers. */
  label: string;
  title: string;
  sub?: string;
  /** A button beside the heading, such as "All tags". */
  action?: ReactNode;
  onClose: () => void;
  children: ReactNode;
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

  // Focus waits until it's placed: before that it's hidden, and hidden things can't take focus.
  // Left in the search, Escape would empty the search as well as closing this.
  const placed = pos !== null;
  useEffect(() => {
    if (placed) panel.current?.focus({ preventScroll: true });
  }, [placed]);

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
      aria-label={label}
      tabIndex={-1}
      style={pos ? { left: pos.left, top: pos.top, width: WIDTH, maxHeight: pos.maxHeight } : { visibility: "hidden" }}
    >
      <div className="filter-head">
        <div>
          <p className="filter-title">{title}</p>
          {sub && <p className="filter-sub">{sub}</p>}
        </div>
        {action}
      </div>
      {children}
    </div>,
    document.body,
  );
}
