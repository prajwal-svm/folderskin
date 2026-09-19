import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type CommunityPack, type Skin } from "../lib/tauri";
import { isTauri } from "../lib/devMock";
import { licenseLabel, PACKS_GUIDE_URL } from "../lib/packs";
import { tagCounts, tagLabel } from "../lib/tags";
import type { ToastTone } from "../hooks/useToasts";
import { Confirm } from "./Confirm";
import { GalleryToolbar, type TabCount } from "./GalleryToolbar";
import { OkBadge } from "./OkBadge";
import { PackPreview, clearPreviews } from "./PackPreview";
import { PackViewer } from "./PackViewer";
import { DeleteIcon } from "./icons/delete";
import { DownloadIcon } from "./icons/download";
import { EyeIcon } from "./icons/eye";
import { FolderOpenIcon } from "./icons/folder-open";
import { LayoutGridIcon } from "./icons/layout-grid";
import { ListIcon } from "./icons/list";
import { LoaderIcon } from "./icons/loader";
import { RefreshCwIcon } from "./icons/refresh-cw";
import { SparklesIcon } from "./icons/sparkles";

type Toast = (text: string, opts?: { tone?: ToastTone; action?: { label: string; run: () => void } }) => void;

/** How the packs are shown: rows with their details, or cards with bigger folders. */
type PackView = "list" | "gallery";

const VIEW_KEY = "folderskin.community.view";

/** The view picked last time, remembered on this computer only. */
function loadView(): PackView {
  try {
    return localStorage.getItem(VIEW_KEY) === "gallery" ? "gallery" : "list";
  } catch {
    return "list";
  }
}

function saveView(view: PackView) {
  try {
    localStorage.setItem(VIEW_KEY, view);
  } catch {
    // Not saving it only means the list comes back next time.
  }
}

/**
 * Skin packs other people shared on GitHub, filtered by tag like the library. Adding a pack
 * puts its skins in the library with the pack's tags; removing it takes them out again.
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
  const [packs, setPacks] = useState<CommunityPack[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tag, setTag] = useState("");
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [removing, setRemoving] = useState<CommunityPack | null>(null);
  const [view, setView] = useState<PackView>(loadView);
  const pickView = (next: PackView) => {
    setView(next);
    saveView(next);
  };
  /** The pack open in the viewer, by id, so it shows its latest state. */
  const [viewing, setViewing] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  /** Goes up with every Refresh, so the previews are downloaded again too. */
  const [generation, setGeneration] = useState(0);

  const load = useCallback(() => {
    setError(null);
    setPacks(null);
    api
      .communityPacks()
      .then(setPacks)
      .catch((e) => {
        setPacks([]);
        setError(errorMessage(e));
      });
  }, []);
  useEffect(load, [load]);

  /** Reads the list from GitHub again, past any cache, and says whether anything changed. */
  const refresh = async () => {
    setRefreshing(true);
    try {
      const list = await api.communityPacks(true);
      clearPreviews();
      setGeneration((g) => g + 1);
      setPacks(list);
      setError(null);
      const updates = list.filter((p) => p.update).length;
      toast(updates ? `${updates} of your packs ${updates === 1 ? "has" : "have"} an update` : "Community is up to date", {
        tone: "ok",
      });
    } catch (e) {
      toast(`Couldn't refresh: ${errorMessage(e)}`, { tone: "danger" });
    } finally {
      setRefreshing(false);
    }
  };

  const tabs = useMemo<TabCount[]>(() => {
    const list = packs ?? [];
    return [{ id: "", label: "All", count: list.length }, ...tagCounts(list).map(({ tag, count }) => ({ id: tag, label: tagLabel(tag), count }))];
  }, [packs]);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (packs ?? []).filter(
      (p) =>
        (!tag || p.tags.includes(tag)) &&
        (!q || p.name.toLowerCase().includes(q) || p.author.toLowerCase().includes(q) || p.tags.some((t) => t.includes(q))),
    );
  }, [packs, tag, query]);

  const mark = (id: string | undefined, added: boolean) =>
    setPacks((ps) => ps?.map((p) => (p.id === id ? { ...p, added, update: false } : p)) ?? ps);

  const add = async (pack: CommunityPack) => {
    setBusy(pack.id);
    try {
      const skins = await api.addPack(pack.id);
      mark(pack.id, true);
      onAdded(skins);
      toast(`Added ${skins.length} skins from ${pack.name}`, {
        tone: "ok",
        action: pack.tags[0] ? { label: "Show", run: () => onShowTag(pack.tags[0]) } : undefined,
      });
    } catch (e) {
      toast(`Couldn't add ${pack.name}: ${errorMessage(e)}`, { tone: "danger" });
    } finally {
      setBusy(null);
    }
  };

  const update = async (pack: CommunityPack) => {
    setBusy(pack.id);
    try {
      const { removed, skins } = await api.updatePack(pack.id);
      if (removed.length) onRemoved(removed);
      onAdded(skins);
      mark(pack.id, true);
      toast(`Updated ${pack.name}`, {
        tone: "ok",
        action: pack.tags[0] ? { label: "Show", run: () => onShowTag(pack.tags[0]) } : undefined,
      });
    } catch (e) {
      toast(`Couldn't update ${pack.name}: ${errorMessage(e)}`, { tone: "danger" });
    } finally {
      setBusy(null);
    }
  };

  const remove = async (pack: CommunityPack) => {
    setRemoving(null);
    setBusy(pack.id);
    try {
      onRemoved(await api.removePack(pack.id));
      mark(pack.id, false);
      toast(`Removed ${pack.name}`, { tone: "ok" });
    } catch (e) {
      toast(`Couldn't remove ${pack.name}: ${errorMessage(e)}`, { tone: "danger" });
    } finally {
      setBusy(null);
    }
  };

  const addFromFolder = async () => {
    const path = isTauri()
      ? await open({ directory: true, multiple: false, title: "Choose a pack folder" }).catch(() => null)
      : "/Users/you/Desktop/my-pack";
    if (typeof path !== "string") return;
    try {
      const skins = await api.importPack(path);
      mark(skins[0]?.pack ?? undefined, true);
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

  return (
    <section className="community">
      <header className="community-head">
        <div className="community-intro">
          <h2 className="view-title">Community</h2>
          <p className="view-sub">Free skins and packs people share on GitHub. Add one and its skins join your library.</p>
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

      {packs && packs.length > 0 && (
        <GalleryToolbar
          tabs={tabs}
          active={tag}
          onChange={setTag}
          query={query}
          onQuery={setQuery}
          label="filter packs by tag"
          placeholder="Search packs"
          extra={
            <>
            <button
              type="button"
              className="icon-btn"
              title="Check GitHub for new and updated packs"
              aria-label="refresh packs"
              aria-busy={refreshing}
              disabled={refreshing}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => void refresh()}
            >
              {refreshing ? <LoaderIcon size={16} /> : <RefreshCwIcon size={16} />}
            </button>
            <div className="view-switch" role="radiogroup" aria-label="show packs as">
              {(
                [
                  ["list", "List", ListIcon],
                  ["gallery", "Gallery", LayoutGridIcon],
                ] as const
              ).map(([id, label, Icon]) => (
                <button
                  key={id}
                  type="button"
                  role="radio"
                  aria-checked={view === id}
                  aria-label={label}
                  title={label}
                  className={view === id ? "view-switch-btn is-active" : "view-switch-btn"}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => pickView(id)}
                >
                  <Icon size={16} />
                </button>
              ))}
            </div>
            </>
          }
        />
      )}

      <div className="community-scroll">
        {packs === null ? (
          <p className="community-note">
            <LoaderIcon /> Loading packs from GitHub…
          </p>
        ) : error ? (
          <div className="empty">
            <span className="empty-glyph">
              <DownloadIcon size={20} />
            </span>
            <p className="empty-title">The packs didn't load</p>
            <p className="empty-text">{error.charAt(0).toUpperCase() + error.slice(1)}. You can still add a pack from a folder.</p>
            <button type="button" className="btn btn-secondary" onClick={load}>
              Try again
            </button>
          </div>
        ) : shown.length === 0 ? (
          <div className="empty">
            <p className="empty-title">{packs.length ? "No pack matches" : "No packs yet"}</p>
            <p className="empty-text">{packs.length ? "Try another word or tag." : "Be the first: share yours."}</p>
          </div>
        ) : (
          <ul className={view === "gallery" ? "packs is-gallery" : "packs"}>
            {shown.map((p, i) => (
              <li key={p.id} className="pack" style={{ animationDelay: `${Math.min(i, 12) * 30}ms` }}>
                <button
                  type="button"
                  className="pack-preview-btn"
                  aria-label={`view ${p.name}`}
                  title={`View ${p.name}`}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => setViewing(p.id)}
                >
                  <PackPreview key={`${p.id}:${generation}`} id={p.id} count={p.count} grid={view === "gallery"} fresh={generation > 0} />
                </button>
                <div className="pack-meta">
                  <p className="pack-name">{p.name}</p>
                  <p className="pack-by">
                    by{" "}
                    <button type="button" className="pack-author" onClick={() => void openUrl(`https://github.com/${p.author}`).catch(() => {})}>
                      @{p.author}
                    </button>{" "}
                    · {p.count} {p.count === 1 ? "skin" : "skins"} · {licenseLabel(p.license)}
                  </p>
                  <div className="pack-tags">
                    {p.tags.slice(0, 4).map((t) => (
                      <button type="button" key={t} className={t === tag ? "tag-chip is-link is-active" : "tag-chip is-link"} onMouseDown={(e) => e.preventDefault()} onClick={() => setTag(t === tag ? "" : t)}>
                        {t}
                      </button>
                    ))}
                  </div>
                </div>
                <div className="pack-action">
                  <button type="button" className="btn btn-secondary" onMouseDown={(e) => e.preventDefault()} onClick={() => setViewing(p.id)}>
                    <EyeIcon size={15} />
                    View
                  </button>
                  {p.added ? (
                    <>
                      {p.update ? (
                        <button
                          type="button"
                          className="btn btn-primary"
                          title="GitHub has a newer version of this pack"
                          disabled={busy !== null}
                          aria-busy={busy === p.id}
                          onMouseDown={(e) => e.preventDefault()}
                          onClick={() => void update(p)}
                        >
                          {busy === p.id ? <LoaderIcon /> : <RefreshCwIcon size={15} />}
                          {busy === p.id ? "Updating…" : "Update"}
                        </button>
                      ) : (
                        <span className="chip chip-ok">
                          <OkBadge size={16} /> Added
                        </span>
                      )}
                      {view === "list" ? (
                        <button type="button" className="btn btn-ghost" disabled={busy === p.id} onMouseDown={(e) => e.preventDefault()} onClick={() => setRemoving(p)}>
                          Remove
                        </button>
                      ) : busy === p.id ? null : (
                        // A card is narrow: the can alone, and gone while the pack updates so
                        // "Updating…" doesn't push it onto a line of its own.
                        <button
                          type="button"
                          className="icon-btn pack-remove"
                          title="Remove this pack"
                          aria-label={`remove ${p.name}`}
                          onMouseDown={(e) => e.preventDefault()}
                          onClick={() => setRemoving(p)}
                        >
                          <DeleteIcon size={16} />
                        </button>
                      )}
                    </>
                  ) : (
                    <button type="button" className="btn btn-primary" disabled={busy !== null} aria-busy={busy === p.id} onMouseDown={(e) => e.preventDefault()} onClick={() => void add(p)}>
                      {busy === p.id ? <LoaderIcon /> : <DownloadIcon size={15} />}
                      {busy === p.id ? "Adding…" : "Add"}
                    </button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        )}
        <p className="community-foot">
          Packs are checked before they're listed.{" "}
          <button type="button" className="link-btn" onClick={() => void openUrl(PACKS_GUIDE_URL).catch(() => {})}>
            How packs work
          </button>
        </p>
      </div>

      {viewing &&
        (() => {
          const pack = packs?.find((p) => p.id === viewing);
          return pack ? (
            <PackViewer
              pack={pack}
              busy={busy === pack.id}
              blocked={busy !== null && busy !== pack.id}
              onAdd={() => void add(pack)}
              onUpdate={() => void update(pack)}
              onRemove={() => setRemoving(pack)}
              onClose={() => setViewing(null)}
            />
          ) : null;
        })()}

      {removing && (
        <Confirm
          title={`Remove "${removing.name}"?`}
          text={`Its skins leave your library. Folders that already use them keep their icon.`}
          action="Remove"
          onCancel={() => setRemoving(null)}
          onConfirm={() => void remove(removing)}
        />
      )}
    </section>
  );
}
