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
import { DownloadIcon } from "./icons/download";
import { FolderOpenIcon } from "./icons/folder-open";
import { LoaderIcon } from "./icons/loader";
import { SparklesIcon } from "./icons/sparkles";

type Toast = (text: string, opts?: { tone?: ToastTone; action?: { label: string; run: () => void } }) => void;

/** Previews downloaded this session, so coming back to Community doesn't load them again. */
const previews = new Map<string, string>();

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

  const mark = (id: string | undefined, added: boolean) => setPacks((ps) => ps?.map((p) => (p.id === id ? { ...p, added } : p)) ?? ps);

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

      {packs && packs.length > 0 && <GalleryToolbar tabs={tabs} active={tag} onChange={setTag} query={query} onQuery={setQuery} label="filter packs by tag" />}

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
          <ul className="packs">
            {shown.map((p, i) => (
              <li key={p.id} className="pack" style={{ animationDelay: `${Math.min(i, 12) * 30}ms` }}>
                <PackPreview id={p.id} />
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
                  {p.added ? (
                    <>
                      <span className="chip chip-ok">
                        <OkBadge size={16} /> Added
                      </span>
                      <button type="button" className="btn btn-ghost" disabled={busy === p.id} onMouseDown={(e) => e.preventDefault()} onClick={() => setRemoving(p)}>
                        Remove
                      </button>
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

/** A pack's preview strip: a few of its skins as folders, loaded once per session. */
function PackPreview({ id }: { id: string }) {
  const [src, setSrc] = useState(() => previews.get(id) ?? null);
  useEffect(() => {
    if (src) return;
    let live = true;
    api
      .communityPreview(id)
      .then((url) => {
        previews.set(id, url);
        if (live) setSrc(url);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [id, src]);
  return <span className="pack-preview">{src ? <img src={src} alt="" draggable={false} /> : <span className="pack-preview-blank" />}</span>;
}
