import { type CSSProperties, useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type CommunityPack, type PackProgress, type PackSkinPreview } from "../lib/tauri";
import { licenseLabel, REPO_URL } from "../lib/packs";
import { progressLabel, progressShare } from "../lib/communityStore";
import { Modal } from "./Modal";
import { OkBadge } from "./OkBadge";
import { DownloadIcon } from "./icons/download";
import { ExternalLinkIcon } from "./icons/external-link";
import { LoaderIcon } from "./icons/loader";
import { RefreshCwIcon } from "./icons/refresh-cw";

/**
 * A community pack opened to look through: every skin drawn as the folder it makes, with its
 * name, and the pack's details. Opened from a skin that matched a search, it starts at that skin.
 * Looking saves nothing; Add does that.
 */
export function PackViewer({
  pack,
  focus = null,
  busy,
  progress = null,
  blocked,
  onAdd,
  onUpdate,
  onRemove,
  onClose,
}: {
  pack: CommunityPack;
  /** The skin to show first, by its place in the pack. */
  focus?: number | null;
  /** This pack is being added, updated or removed. */
  busy: boolean;
  /** How far adding it has got. */
  progress?: PackProgress | null;
  /** Another pack is. */
  blocked: boolean;
  onAdd: () => void;
  onUpdate: () => void;
  onRemove: () => void;
  onClose: () => void;
}) {
  const [skins, setSkins] = useState<PackSkinPreview[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const focused = useRef<HTMLLIElement>(null);

  useEffect(() => {
    let live = true;
    api
      .packSkins(pack.id, pack.hash)
      .then((list) => live && setSkins(list))
      .catch((e) => live && setError(errorMessage(e)));
    return () => {
      live = false;
    };
  }, [pack.id, pack.hash]);

  useEffect(() => {
    if (skins && focus !== null) focused.current?.scrollIntoView({ block: "center" });
  }, [skins, focus]);

  const adding = busy && !pack.added;
  const primary = !pack.added ? (
    <button
      type="button"
      className={adding ? "btn btn-primary pack-adding" : "btn btn-primary"}
      disabled={busy || blocked}
      aria-busy={busy}
      style={adding ? ({ "--done": progressShare(progress) } as CSSProperties) : undefined}
      onClick={onAdd}
    >
      {busy ? <LoaderIcon /> : <DownloadIcon size={15} />}
      {busy ? progressLabel(progress) : `Add ${pack.count} ${pack.count === 1 ? "skin" : "skins"}`}
    </button>
  ) : pack.update ? (
    <button type="button" className="btn btn-primary" disabled={busy || blocked} aria-busy={busy} onClick={onUpdate}>
      {busy ? <LoaderIcon /> : <RefreshCwIcon size={15} />}
      {busy ? "Updating" : "Update"}
    </button>
  ) : (
    <span className="chip chip-ok">
      <OkBadge size={16} /> Added
    </span>
  );

  return (
    <Modal
      wide
      title={pack.name}
      sub={
        <>
          by{" "}
          <button type="button" className="pack-author" onClick={() => void openUrl(`https://github.com/${pack.author}`).catch(() => {})}>
            @{pack.author}
          </button>{" "}
          · {pack.count} {pack.count === 1 ? "skin" : "skins"} · {licenseLabel(pack.license)}
          {pack.bytes > 0 && <> · {megabytes(pack.bytes)}</>}
        </>
      }
      onClose={onClose}
      footer={
        <>
          <button
            type="button"
            className="link-btn pack-view-github"
            onClick={() => void openUrl(`${REPO_URL}/tree/main/community/packs/${pack.id}`).catch(() => {})}
          >
            Open on GitHub <ExternalLinkIcon size={13} />
          </button>
          {pack.added && (
            <button type="button" className="btn btn-ghost" disabled={busy || blocked} onClick={onRemove}>
              Remove
            </button>
          )}
          {primary}
        </>
      }
    >
      {pack.tags.length > 0 && (
        <div className="pack-tags">
          {pack.tags.map((t) => (
            <span key={t} className="tag-chip">
              {t}
            </span>
          ))}
        </div>
      )}
      {error ? (
        <p className="field-note is-error">Couldn't open this pack: {error}.</p>
      ) : skins === null ? (
        <p className="community-note">
          <LoaderIcon /> Getting {pack.count} {pack.count === 1 ? "skin" : "skins"} to show them
        </p>
      ) : (
        <ul className="pack-skins">
          {skins.map((s, i) => (
            <li
              key={`${i}:${s.name}`}
              ref={i === focus ? focused : undefined}
              className={i === focus ? "pack-skin is-focus" : "pack-skin"}
              style={{ animationDelay: `${Math.min(i, 16) * 25}ms` }}
            >
              <img src={s.thumbnail} alt="" draggable={false} loading="lazy" decoding="async" />
              <span className="pack-skin-name" title={s.name}>
                {s.name}
              </span>
            </li>
          ))}
        </ul>
      )}
    </Modal>
  );
}

/** "3.2 MB", "640 KB". */
function megabytes(bytes: number): string {
  return bytes >= 1_000_000 ? `${(bytes / 1_000_000).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1000))} KB`;
}
