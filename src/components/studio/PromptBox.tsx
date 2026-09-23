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
    /** Why sending can't happen right now, when it can't though there are words to send. */
    blocked: string | null;
    /** A picture is being dragged over the window: it can be dropped here as a reference. */
    dropping: boolean;
    boxRef?: React.Ref<HTMLFormElement>;
  }
>(function PromptBox(
  { idea, onIdea, placeholder, rows, refs, adding, onAddRef, onRemoveRef, shape, onShape, provider, model, onSettings, onSend, blocked, dropping, boxRef },
  ref,
) {
  const limit = refLimit(provider, model);
  const ready = provider?.kind === "local" ? provider.has_key : Boolean(provider?.has_key);
  const canSend = Boolean(idea.trim()) && Boolean(model) && !blocked;
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
  const status = provider?.kind === "local" ? (ready ? "Set up and ready" : "Not set up yet") : ready ? "Key saved" : "No key yet";
  return (
    <form className={dropping ? "composer is-drop-target" : "composer"} onSubmit={submit} ref={boxRef}>
      <textarea
        ref={ref}
        className="composer-input"
        rows={rows}
        value={idea}
        placeholder={dropping ? "Drop the picture to paint from it" : placeholder}
        aria-label="describe the folder"
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
                data-tip={unused ? (limit === 0 ? `${model?.label ?? "This model"} can't paint from a picture, so this one won't be sent.` : `${model?.label ?? "This model"} takes ${limit === 1 ? "one picture" : `${limit} pictures`}, so this one won't be sent.`) : r.name}
              >
                <img src={r.thumb} alt="" draggable={false} />
                <span className="composer-ref-name">{r.name}</span>
                <button type="button" className="composer-ref-x" aria-label={`remove ${r.name}`} data-tip="Remove" onClick={() => onRemoveRef(r.id)}>
                  <XIcon size={12} />
                </button>
              </span>
            );
          })}
          {adding && (
            <span className="composer-ref is-adding">
              <LoaderIcon size={13} /> Adding the picture
            </span>
          )}
        </div>
      )}
      <div className="composer-bar">
        <Segmented<Shape>
          label="what to make"
          small
          value={shape}
          onChange={onShape}
          options={[
            { value: "folder", label: "Whole folder", title: "The model paints the whole folder, like a poster. Shapes can vary a little." },
            { value: "skin", label: "Just the art", title: "The model paints flat art; FolderSkin wraps it onto its own folder." },
          ]}
        />
        <button
          type="button"
          className="icon-btn composer-attach"
          aria-label="add a reference picture"
          disabled={limit === 0}
          data-tip={limit === 0 ? `${model?.label ?? "This model"} can't paint from a picture` : "Add a picture to paint from (or drop one here)"}
          onClick={onAddRef}
        >
          <PaperclipIcon />
        </button>
        <span className="composer-spacer" />
        <button type="button" className="model-pill" onClick={onSettings} data-tip={`${status}. Choose the provider and model`}>
          <span className={ready ? "model-dot is-ready" : "model-dot"} aria-hidden="true" />
          {provider?.kind === "local" ? <CpuIcon size={14} /> : provider && <ProviderLogo id={provider.id} size={14} />}
          <span className="model-pill-text">{provider ? `${provider.label} · ${model?.label ?? ""}` : "Choose a provider"}</span>
          <SlidersHorizontalIcon size={14} />
        </button>
        <button type="submit" className="send-btn" disabled={!canSend} aria-label="generate" data-tip={blocked ?? "Generate"} data-tip-kbd={blocked ? undefined : "Enter"}>
          <ArrowUpIcon size={17} />
        </button>
      </div>
    </form>
  );
});
