import { useMemo, useState } from "react";
import { chatPrompt, STYLES } from "../lib/prompts";
import type { ToastTone } from "../hooks/useToasts";
import { Modal } from "./Modal";
import { CheckIcon } from "./icons/check";
import { CopyIcon } from "./icons/copy";

/**
 * No API key needed: paint the folder in Grok's or ChatGPT's own chat. Three steps, with the
 * prompt already filled in from whatever the user typed and picked in the studio.
 */
export function ChatHelper({
  scene,
  styleId,
  onImport,
  onClose,
  toast,
}: {
  scene: string;
  styleId: string | null;
  onImport: () => void;
  onClose: () => void;
  toast: (text: string, opts?: { tone?: ToastTone }) => void;
}) {
  const [style, setStyle] = useState<string | null>(styleId);
  const [copied, setCopied] = useState(false);
  const prompt = useMemo(() => chatPrompt(scene, style), [scene, style]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(prompt);
      setCopied(true);
      toast("Prompt copied", { tone: "ok" });
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      toast("Couldn't reach the clipboard. Select the text and copy it instead.", { tone: "danger" });
    }
  };

  return (
    <Modal
      wide
      title="Make it in Grok or ChatGPT"
      sub="Their chat apps can paint a folder for you. FolderSkin cuts it out afterwards."
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Close
          </button>
          <button type="button" className="btn btn-primary" onClick={onImport}>
            Add the finished picture
          </button>
        </>
      }
    >
      <ol className="helper-steps">
        <li>
          <div className="helper-step-text">
            <p className="helper-step-title">Attach this template to your message</p>
            <p className="field-note">Drag it into the chat window. It keeps every folder the same shape.</p>
          </div>
          <img className="helper-template" src="/folder-template.png" alt="blank FolderSkin folder template" draggable />
        </li>
        <li>
          <div className="helper-step-text">
            <p className="helper-step-title">Paste the prompt</p>
            <div className="helper-styles">
              {STYLES.map((s) => (
                <button
                  key={s.id}
                  type="button"
                  className={style === s.id ? "style-chip is-active" : "style-chip"}
                  aria-pressed={style === s.id}
                  onClick={() => setStyle((cur) => (cur === s.id ? null : s.id))}
                >
                  {s.label}
                </button>
              ))}
            </div>
            <pre className="helper-prompt">{prompt}</pre>
            <button type="button" className="btn btn-secondary helper-copy" onClick={copy}>
              {copied ? <CheckIcon size={14} playOnMount /> : <CopyIcon size={14} />}
              {copied ? "Copied" : "Copy prompt"}
            </button>
          </div>
        </li>
        <li>
          <div className="helper-step-text">
            <p className="helper-step-title">Save the picture and add it here</p>
            <p className="field-note">
              The magenta goes, the folder lands in Yours. If the edges look pink, ask the chat to repaint the background
              as exactly #FF00FF.
            </p>
          </div>
        </li>
      </ol>
    </Modal>
  );
}
