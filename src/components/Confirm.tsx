import { useLayoutEffect, useRef } from "react";
import { useT } from "../i18n";
import { Modal } from "./Modal";
import { DeleteIcon } from "./icons/delete";
import { branded } from "./Brand";

/**
 * Holds an element at the largest `axis` it has had while the dialog is open, so words that
 * change while it asks (a count still going) never move the buttons under the pointer.
 */
function useSteady<T extends HTMLElement>(words: string, axis: "height" | "width") {
  const ref = useRef<T>(null);
  const most = useRef(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const prop = axis === "height" ? "minHeight" : "minWidth";
    el.style[prop] = "";
    most.current = Math.max(most.current, el.getBoundingClientRect()[axis]);
    el.style[prop] = `${most.current}px`;
  }, [words, axis]);
  return ref;
}

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
  const t = useT();
  const textRef = useSteady<HTMLParagraphElement>(text, "height");
  const actionRef = useSteady<HTMLButtonElement>(action, "width");
  return (
    <Modal
      narrow
      title={title}
      onClose={onCancel}
      footer={
        <>
          <button type="button" className="btn btn-ghost" onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <button type="button" ref={actionRef} className={tone === "danger" ? "btn btn-danger" : "btn btn-primary"} onClick={onConfirm}>
            {tone === "danger" && <DeleteIcon size={15} />}
            {action}
          </button>
        </>
      }
    >
      <div className="confirm">
        {image && <img className="confirm-thumb" src={image} alt="" draggable={false} />}
        <p className="confirm-text" ref={textRef}>
          {branded(text)}
        </p>
      </div>
    </Modal>
  );
}
