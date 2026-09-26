import { useMemo, useState } from "react";
import { chatPrompt, STYLES } from "../lib/prompts";
import { styleName } from "../lib/styles";
import type { ToastTone } from "../hooks/useToasts";
import { Modal } from "./Modal";
import { CheckIcon } from "./icons/check";
import { CopyIcon } from "./icons/copy";
import { t as tNow, useT } from "../i18n";

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
  const t = useT();
  const [style, setStyle] = useState<string | null>(styleId);
  const [copied, setCopied] = useState(false);
  const prompt = useMemo(() => chatPrompt(scene, style), [scene, style]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(prompt);
      setCopied(true);
      toast(tNow("ai.helper.copiedToast"), { tone: "ok" });
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      toast(tNow("ai.helper.clipboardFailed"), { tone: "danger" });
    }
  };

  return (
    <Modal
      wide
      title={t("ai.helper.title")}
      sub={t("ai.helper.sub")}
      onClose={onClose}
      footer={
        <button type="button" className="btn btn-primary" onClick={onImport}>
          {t("ai.helper.add")}
        </button>
      }
    >
      <ol className="helper-steps">
        <li>
          <div className="helper-step-text">
            <p className="helper-step-title">{t("ai.helper.attach")}</p>
            <p className="field-note">{t("ai.helper.attachNote")}</p>
          </div>
          <img className="helper-template" src="/folder-template.png" alt={t("ai.helper.templateAlt")} draggable />
        </li>
        <li>
          <div className="helper-step-text">
            <p className="helper-step-title">{t("ai.helper.paste")}</p>
            <div className="helper-styles">
              {STYLES.map((s) => (
                <button
                  key={s.id}
                  type="button"
                  className={style === s.id ? "style-chip is-active" : "style-chip"}
                  aria-pressed={style === s.id}
                  onClick={() => setStyle((cur) => (cur === s.id ? null : s.id))}
                >
                  {styleName(s.id)}
                </button>
              ))}
            </div>
            <pre className="helper-prompt">{prompt}</pre>
            <button type="button" className="btn btn-secondary helper-copy" onClick={copy}>
              {copied ? <CheckIcon size={14} playOnMount /> : <CopyIcon size={14} />}
              {copied ? t("ai.helper.copied") : t("ai.helper.copy")}
            </button>
          </div>
        </li>
        <li>
          <div className="helper-step-text">
            <p className="helper-step-title">{t("ai.helper.save")}</p>
            <p className="field-note">{t("ai.helper.saveNote")}</p>
          </div>
        </li>
      </ol>
    </Modal>
  );
}
