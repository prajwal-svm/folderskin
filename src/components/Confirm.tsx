import { Modal } from "./Modal";
import { DeleteIcon } from "./icons/delete";

/**
 * "Are you sure?" before something can't be undone, like deleting a skin or removing a pack, or
 * before something big, like giving a whole tree of folders a skin (`tone: "primary"`).
 * Cancel comes first and gets the focus, so Return keeps things as they are.
 */
export function Confirm({
  title,
  text,
  image,
  action,
  tone = "danger",
  onCancel,
  onConfirm,
}: {
  title: string;
  text: string;
  /** A picture of what goes, beside the text. */
  image?: string;
  /** The button that does it, such as "Delete". */
  action: string;
  /** Danger for what can't be undone; primary for a big step that can be. */
  tone?: "danger" | "primary";
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <Modal
      narrow
      title={title}
      onClose={onCancel}
      footer={
        <>
          <button type="button" className="btn btn-ghost" onClick={onCancel}>
            Cancel
          </button>
          <button type="button" className={tone === "danger" ? "btn btn-danger" : "btn btn-primary"} onClick={onConfirm}>
            {tone === "danger" && <DeleteIcon size={15} />}
            {action}
          </button>
        </>
      }
    >
      <div className="confirm">
        {image && <img className="confirm-thumb" src={image} alt="" draggable={false} />}
        <p className="confirm-text">{text}</p>
      </div>
    </Modal>
  );
}
