import { useState } from "react";
import type { ChatFolder } from "../../state/chats";
import { clip } from "../../lib/names";
import { Popover } from "../composer/Popover";
import { ChevronDownIcon, FolderIcon, XIcon } from "../icons/composer";
import { FolderOpenIcon } from "../icons/folder-open";
import { HardDriveIcon } from "../icons/hard-drive";
import { useT } from "../../i18n";

/**
 * The folder the chat's pictures are for, in the chat's header: choose one, show or hide it on the
 * right, or stop using one. The folder panel on the right comes and goes with it. It says drive
 * when the pictures are for a drive.
 */
export function FolderTarget({
  folder,
  drive,
  panelShown,
  onChoose,
  onTogglePanel,
  onClear,
}: {
  folder: ChatFolder | null;
  /** The pictures are for a drive: the drive chosen, or with none chosen yet, a drive's shape. */
  drive: boolean;
  panelShown: boolean;
  onChoose: () => void;
  onTogglePanel: () => void;
  onClear: () => void;
}) {
  const t = useT();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const Icon = drive ? HardDriveIcon : FolderIcon;
  if (!folder) {
    return (
      <button type="button" className="folder-target is-empty" data-tip={t(drive ? "ai.target.chooseDriveTip" : "ai.target.chooseTip")} onClick={onChoose}>
        <Icon size={15} />
        {drive ? t("ai.target.chooseDrive") : t("common.dialog.chooseFolder")}
      </button>
    );
  }
  const close = () => setAnchor(null);
  return (
    <>
      <button
        type="button"
        className="folder-target"
        aria-haspopup="menu"
        aria-expanded={anchor !== null}
        aria-label={t(drive ? "ai.target.labelDrive" : "ai.target.label", { name: folder.name })}
        data-tip={folder.path}
        onClick={(e) => setAnchor(anchor ? null : e.currentTarget)}
      >
        <Icon size={15} />
        <span className="folder-target-text">
          <span className="folder-target-label">{t("ai.target.for")}</span> {clip(folder.name, 28)}
        </span>
        <ChevronDownIcon size={13} />
      </button>
      {anchor && (
        <Popover anchor={anchor} onClose={close} width={240} label={t(drive ? "ai.target.menuDrive" : "ai.target.menu")} align="end">
          <div className="cmp-menu" role="menu">
            <button
              type="button"
              role="menuitem"
              className="menu-item"
              onClick={() => {
                close();
                onChoose();
              }}
            >
              <FolderOpenIcon size={15} />
              {t(drive ? "ai.target.anotherDrive" : "ai.target.another")}
            </button>
            <button
              type="button"
              role="menuitem"
              className="menu-item"
              onClick={() => {
                close();
                onTogglePanel();
              }}
            >
              <Icon size={15} />
              {panelShown ? t("ai.target.hide") : t("ai.target.show")}
            </button>
            <button
              type="button"
              role="menuitem"
              className="menu-item"
              onClick={() => {
                close();
                onClear();
              }}
            >
              <XIcon size={15} />
              {t(drive ? "ai.target.noneDrive" : "ai.target.none")}
            </button>
          </div>
        </Popover>
      )}
    </>
  );
}
