import type { Skin } from "../lib/tauri";
import { Modal } from "./Modal";

/**
 * The "are you sure?" before one of the user's skins is deleted for good. Cancel comes first and
 * gets the focus, so Return keeps the skin.
 */
export function ConfirmDelete({ skin, onCancel, onDelete }: { skin: Skin; onCancel: () => void; onDelete: () => void }) {
  return (
    <Modal
      narrow
      title={`Delete "${skin.name}"?`}
      onClose={onCancel}
      footer={
        <>
          <button type="button" className="btn btn-ghost" onClick={onCancel}>
            Cancel
          </button>
          <button type="button" className="btn btn-danger" onClick={onDelete}>
            Delete
          </button>
        </>
      }
    >
      <div className="confirm">
        <img className="confirm-thumb" src={skin.thumbnail} alt="" draggable={false} />
        <p className="confirm-text">This removes it from Yours for good. Folders that already use it keep their icon.</p>
      </div>
    </Modal>
  );
}
