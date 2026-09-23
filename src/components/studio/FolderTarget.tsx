import { useState } from "react";
import type { ChatFolder } from "../../state/chats";
import { clip } from "../../lib/names";
import { Popover } from "../composer/Popover";
import { ChevronDownIcon, FolderIcon, XIcon } from "../icons/composer";
import { FolderOpenIcon } from "../icons/folder-open";

/**
 * The folder the chat's pictures are for, in the chat's header: choose one, show or hide it on the
 * right, or stop using one. The folder panel on the right comes and goes with it.
 */
export function FolderTarget({
  folder,
  panelShown,
  onChoose,
  onTogglePanel,
  onClear,
}: {
  folder: ChatFolder | null;
  panelShown: boolean;
  onChoose: () => void;
  onTogglePanel: () => void;
  onClear: () => void;
}) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  if (!folder) {
    return (
      <button type="button" className="folder-target is-empty" data-tip="Choose the folder these pictures are for" onClick={onChoose}>
        <FolderIcon size={15} />
        Choose a folder
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
        aria-label={`for the folder ${folder.name}`}
        data-tip={folder.path}
        onClick={(e) => setAnchor(anchor ? null : e.currentTarget)}
      >
        <FolderIcon size={15} />
        <span className="folder-target-text">
          <span className="folder-target-label">For</span> {clip(folder.name, 28)}
        </span>
        <ChevronDownIcon size={13} />
      </button>
      {anchor && (
        <Popover anchor={anchor} onClose={close} width={240} label="The folder" align="end">
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
              Choose another folder…
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
              <FolderIcon size={15} />
              {panelShown ? "Hide it on the right" : "Show it on the right"}
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
              Don't use a folder
            </button>
          </div>
        </Popover>
      )}
    </>
  );
}
