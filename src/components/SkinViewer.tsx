import type { CollectionSkin } from "../lib/tauri";
import { Modal } from "./Modal";
import { OfficialBadge } from "./OfficialBadge";
import { OkBadge } from "./OkBadge";
import { DownloadIcon } from "./icons/download";
import { LoaderIcon } from "./icons/loader";
import { licenseLabel } from "../lib/packs";
import { useT } from "../i18n";
import { formatBytes } from "../i18n/format";

/**
 * One of the official skins opened on its own: the folder it makes, big, with its name and tags.
 * Official skins aren't in a pack, so there is nothing to add: Use this skin saves this one into
 * the library and takes it to where Apply is. One in the library already says so, with the tick,
 * and takes you to it there.
 */
export function SkinViewer({
  skin,
  license,
  using,
  blocked,
  kept = false,
  onUse,
  onShow,
  onClose,
}: {
  skin: CollectionSkin;
  /** The licence every official skin comes under; empty when the collection names none. */
  license: string;
  /** This skin is being saved. */
  using: boolean;
  /** Another skin is. */
  blocked: boolean;
  /** It's in the library already. */
  kept?: boolean;
  onUse: () => void;
  /** Picks it in the library, where Apply is: for a skin there already. */
  onShow: () => void;
  onClose: () => void;
}) {
  const t = useT();
  return (
    <Modal
      narrow
      title={skin.name}
      sub={
        <>
          {t("community.official.collection")}
          {license && <> · {licenseLabel(license)}</>}
          {skin.bytes > 0 && <> · {formatBytes(skin.bytes)}</>}
        </>
      }
      onClose={onClose}
      footer={
        kept ? (
          <button type="button" className="btn btn-primary is-done" data-tip={t("community.inLibrary.showTip")} onClick={onShow}>
            <OkBadge size={16} />
            {t("community.inLibrary.button")}
          </button>
        ) : (
          <button type="button" className="btn btn-primary" disabled={using || blocked} aria-busy={using} onClick={onUse}>
            {using ? <LoaderIcon /> : <DownloadIcon size={15} />}
            {using ? t("community.progress.adding") : t("community.use.button")}
          </button>
        )
      }
    >
      <div className="pack-tags">
        <OfficialBadge />
        {skin.tags.map((tag) => (
          <span key={tag} className="tag-chip">
            {tag}
          </span>
        ))}
      </div>
      <div className="skin-view">
        <img src={skin.thumbnail} alt="" draggable={false} decoding="async" />
      </div>
    </Modal>
  );
}
