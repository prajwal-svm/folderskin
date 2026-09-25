import { forwardRef, type FormEvent, type KeyboardEvent } from "react";
import type { AiModel, AiProvider } from "../../lib/tauri";
import type { ChatRef, Shape } from "../../state/chats";
import { ProviderLogo } from "../ProviderLogo";
import { Segmented } from "../composer/controls";
import { ArrowUpIcon } from "../icons/arrow-up";
import { CpuIcon, XIcon } from "../icons/composer";
import { LoaderIcon } from "../icons/loader";
import { PaperclipIcon } from "../icons/paperclip";
import { SlidersHorizontalIcon } from "../icons/sliders-horizontal";
import { useT } from "../../i18n";

/** How many reference pictures a model takes: several on this computer, one elsewhere, none when it can't. */
export function refLimit(provider: AiProvider | undefined, model: AiModel | undefined): number {
  if (!model?.accepts_reference) return 0;
  return provider?.kind === "local" ? 4 : 1;
}

/**
 * Where an idea is written: the words, the reference pictures (a picture a model can't take is
 * marked and left out, never silently dropped), whole folder or just the art, the model it goes
 * to (which opens the provider settings) and send. Enter sends, Shift+Enter starts a new line.
 */
export const PromptBox = forwardRef<
  HTMLTextAreaElement,
  {
    idea: string;
    onIdea: (idea: string) => void;
    placeholder: string;
    rows: number;
    refs: ChatRef[];
    /** A reference picture is being copied in. */
    adding: boolean;
    onAddRef: () => void;
    onRemoveRef: (id: string) => void;
    shape: Shape;
    onShape: (shape: Shape) => void;
    provider: AiProvider | undefined;
    model: AiModel | undefined;
    onSettings: () => void;
    onSend: () => void;
    /** The providers aren't in yet: sending waits for them. */
    loading: boolean;
    /** Sent while they weren't, and waiting for them. */
    queued: boolean;
    /** Why sending can't happen right now, when it can't though there are words to send. */
    blocked: string | null;
    /** A picture is being dragged over the window: it can be dropped here as a reference. */
    dropping: boolean;
    boxRef?: React.Ref<HTMLFormElement>;
  }
>(function PromptBox(
  { idea, onIdea, placeholder, rows, refs, adding, onAddRef, onRemoveRef, shape, onShape, provider, model, onSettings, onSend, loading, queued, blocked, dropping, boxRef },
  ref,
) {
  const t = useT();
  const limit = refLimit(provider, model);
  const ready = provider?.kind === "local" ? provider.has_key : Boolean(provider?.has_key);
  const canSend = Boolean(idea.trim()) && (Boolean(model) || loading) && !blocked && !queued;
  const submit = (e?: FormEvent) => {
    e?.preventDefault();
    if (canSend) onSend();
  };
  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      submit();
    }
  };
  const status = provider?.kind === "local" ? (ready ? t("ai.prompt.status.localReady") : t("ai.prompt.status.localNotReady")) : ready ? t("ai.prompt.status.keySaved") : t("ai.prompt.status.noKey");
  const where = `${provider?.label ?? ""} · ${model?.label ?? ""}`;
  const modelName = model?.label ?? t("ai.prompt.thisModel");
  return (
    <form className={dropping ? "composer is-drop-target" : "composer"} onSubmit={submit} ref={boxRef}>
      <textarea
        ref={ref}
        className="composer-input"
        rows={rows}
        value={idea}
        placeholder={dropping ? t("ai.prompt.dropRef") : placeholder}
        aria-label={t("ai.prompt.label")}
        spellCheck
        onChange={(e) => onIdea(e.target.value)}
        onKeyDown={onKey}
      />
      {(refs.length > 0 || adding) && (
        <div className="composer-refs">
          {refs.map((r, i) => {
            const unused = i >= limit;
            return (
              <span
                key={r.id}
                className={unused ? "composer-ref is-unused" : "composer-ref"}
                data-tip={unused ? (limit === 0 ? t("ai.prompt.noRefs", { model: modelName }) : t("ai.prompt.refLimit", { model: modelName, count: limit })) : r.name}
              >
                <img src={r.thumb} alt="" draggable={false} />
                <span className="composer-ref-name">{r.name}</span>
                <button type="button" className="composer-ref-x" aria-label={t("ai.prompt.removeRefLabel", { name: r.name })} data-tip={t("ai.prompt.removeRef")} onClick={() => onRemoveRef(r.id)}>
                  <XIcon size={12} />
                </button>
              </span>
            );
          })}
          {adding && (
            <span className="composer-ref is-adding">
              <LoaderIcon size={13} /> {t("ai.prompt.addingRef")}
            </span>
          )}
        </div>
      )}
      <div className="composer-bar">
        <Segmented<Shape>
          label={t("ai.prompt.shapeLabel")}
          small
          value={shape}
          onChange={onShape}
          options={[
            { value: "folder", label: t("ai.prompt.shape.folder"), title: t("ai.prompt.shape.folderTip") },
            { value: "skin", label: t("ai.prompt.shape.skin"), title: t("ai.prompt.shape.skinTip") },
          ]}
        />
        <button
          type="button"
          className="icon-btn composer-attach"
          aria-label={t("ai.prompt.addRefLabel")}
          disabled={limit === 0}
          data-tip={limit === 0 ? t("ai.prompt.cantRef", { model: modelName }) : t("ai.prompt.addRef")}
          onClick={onAddRef}
        >
          <PaperclipIcon />
        </button>
        <span className="composer-spacer" />
        <button type="button" className="model-pill" onClick={onSettings} data-tip={provider ? t("ai.prompt.pillTip", { where, status: status.charAt(0).toLocaleLowerCase() + status.slice(1) }) : t("ai.prompt.choose")}>
          <span className={ready ? "model-dot is-ready" : "model-dot"} aria-hidden="true" />
          {provider?.kind === "local" ? <CpuIcon size={14} /> : provider ? <ProviderLogo id={provider.id} size={14} /> : loading && <LoaderIcon size={14} />}
          <span className="model-pill-text">{provider ? where : loading ? t("ai.prompt.loading") : t("ai.prompt.chooseProvider")}</span>
          <SlidersHorizontalIcon size={14} />
        </button>
        <button
          type="submit"
          className="send-btn"
          disabled={!canSend}
          aria-label={t("ai.prompt.generateLabel")}
          aria-busy={queued || undefined}
          data-tip={blocked ?? (queued ? t("ai.prompt.queued") : t("ai.prompt.generate"))}
          data-tip-kbd={blocked || queued ? undefined : "Enter"}
        >
          {queued ? <LoaderIcon size={16} /> : <ArrowUpIcon size={17} />}
        </button>
      </div>
    </form>
  );
});
