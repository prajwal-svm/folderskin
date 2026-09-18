import { Modal } from "./Modal";

/**
 * "Are you sure?" before something can't be undone, like deleting a skin or removing a pack.
 * Cancel comes first and gets the focus, so Return keeps things as they are.
 */
export function Confirm({
  title,
  text,
  image,
  action,
  onCancel,
  onConfirm,
}: {
  title: string;
  text: string;
  /** A picture of what goes, beside the text. */
  image?: string;
  /** The button that does it, such as "Delete". */
  action: string;
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
          <button type="button" className="btn btn-danger" onClick={onConfirm}>
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
