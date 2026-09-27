import type { ReactNode } from "react";
import type { Skin } from "../lib/tauri";
import type { Os } from "../lib/platform";
import { exportKinds, type ExportKind } from "../lib/exports";
import { clip } from "../lib/names";
import { useT } from "../i18n";
import { Modal } from "./Modal";
import { AppleMark, WindowsMark } from "./LookSwitch";
import { EarthIcon } from "./icons/earth";
import { FolderOpenIcon } from "./icons/folder-open";
import { ImageIcon } from "./icons/image";
import { LayoutGridIcon } from "./icons/layout-grid";
import { svgProps } from "./icons/trigger";

/** Lucide's `smartphone`, standing for an iPhone or iPad app icon. */
function PhoneGlyph() {
  return (
    <svg {...svgProps(16)}>
      <rect width="14" height="20" x="5" y="2" rx="2" ry="2" />
      <path d="M12 18h.01" />
    </svg>
  );
}

const GLYPHS: Record<ExportKind, ReactNode> = {
  icns: <AppleMark />,
  ico: <WindowsMark />,
  png: <ImageIcon size={16} />,
  jpeg: <ImageIcon size={16} />,
  iconset: <LayoutGridIcon size={16} />,
  ios: <PhoneGlyph />,
  favicon: <EarthIcon size={16} />,
  folder: <FolderOpenIcon size={16} />,
};

/**
 * Saves a skin on its own, to use anywhere: every kind of file an icon goes in, each one click
 * away. Picking one closes this and asks where it goes.
 */
export function ExportSkin({ skin, os, onPick, onClose }: { skin: Skin; os: Os; onPick: (kind: ExportKind) => void; onClose: () => void }) {
  const t = useT();
  return (
    <Modal title={t("library.export.title", { name: clip(skin.name) })} sub={t("library.export.sub")} onClose={onClose}>
      <div className="export-kinds">
        {exportKinds(os).map((kind) => (
          <button key={kind} type="button" className="export-kind" onClick={() => onPick(kind)}>
            <span className="export-kind-glyph" aria-hidden="true">
              {GLYPHS[kind]}
            </span>
            <span className="export-kind-text">
              <span className="export-kind-name">{t(`library.export.kinds.${kind}.name`)}</span>
              <span className="export-kind-hint">{t(`library.export.kinds.${kind}.hint`)}</span>
            </span>
          </button>
        ))}
      </div>
    </Modal>
  );
}
