import { useCallback, useEffect, useMemo, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errorMessage, type AiCatalogue, type AiModel, type AiProvider, type Skin } from "../lib/tauri";
import { isTauri } from "../lib/devMock";
import { baseName, IMAGE_EXTENSIONS } from "../lib/files";
import { SCENES, STYLES, withStyle } from "../lib/prompts";
import type { ToastTone } from "../hooks/useToasts";
import { FolderGhost } from "./FolderGhost";
import { StudioSettings } from "./StudioSettings";
import { ChatHelper } from "./ChatHelper";
import { ArrowUpIcon } from "./icons/arrow-up";
import { PaperclipIcon } from "./icons/paperclip";
import { SlidersHorizontalIcon } from "./icons/sliders-horizontal";
import { SparklesIcon } from "./icons/sparkles";

type Shape = "folder" | "skin";

type Turn = {
  id: number;
  idea: string;
  styleId: string | null;
  shape: Shape;
  where: string;
  status: "working" | "done" | "error";
  started: number;
  skin?: Skin;
  error?: string;
};

/** What the status line says while a picture is on its way, in order. */
const STEPS: Record<Shape, string[]> = {
  folder: ["Sending your idea", "Sketching the scene", "Painting the folder", "Cutting it out of the background"],
  skin: ["Sending your idea", "Sketching the scene", "Painting the artwork", "Wrapping it onto a folder"],
};

/**
 * The AI assistant. An empty studio is a composer in the middle of the island; once the first
 * idea is sent the composer settles at the bottom and each request becomes a card above it
 * that "develops" into a folder you can try on straight away. Provider, model and key live in
 * a settings dialog, out of the way until they are needed.
 */
export function Studio({
  folderName,
  selectedId,
  onGenerated,
  onTryOn,
  onImport,
  toast,
}: {
  folderName: string | null;
  selectedId: string | null;
  onGenerated: (skin: Skin) => void;
  onTryOn: (skinId: string) => void;
  onImport: () => void;
  toast: (text: string, opts?: { tone?: ToastTone }) => void;
}) {
  const [catalogue, setCatalogue] = useState<AiCatalogue | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [providerId, setProviderId] = useState("");
  const [modelId, setModelId] = useState("");
  const [idea, setIdea] = useState("");
  const [styleId, setStyleId] = useState<string | null>(null);
  const [shape, setShape] = useState<Shape>("folder");
  const [reference, setReference] = useState<string | null>(null);
  const [turns, setTurns] = useState<Turn[]>([]);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [helperOpen, setHelperOpen] = useState(false);
  const input = useRef<HTMLTextAreaElement>(null);
  const thread = useRef<HTMLDivElement>(null);
  const seq = useRef(0);

  const load = useCallback(() => {
    setLoadError(null);
    api
      .aiCatalogue()
      .then((c) => {
        setCatalogue(c);
        setProviderId((p) => p || c.providers.find((x) => x.has_key)?.id || c.providers[0]?.id || "");
      })
      .catch((e) => setLoadError(errorMessage(e)));
  }, []);
  useEffect(load, [load]);

  const provider: AiProvider | undefined = useMemo(
    () => catalogue?.providers.find((p) => p.id === providerId),
    [catalogue, providerId],
  );
  const model: AiModel | undefined = useMemo(
    () => provider?.models.find((m) => m.id === modelId) ?? provider?.models[0],
    [provider, modelId],
  );
  useEffect(() => {
    if (provider && !provider.models.some((m) => m.id === modelId)) setModelId(provider.models[0]?.id ?? "");
  }, [provider, modelId]);

  const working = turns.some((t) => t.status === "working");
  const hasThread = turns.length > 0;
  const canSend = Boolean(idea.trim()) && !working && Boolean(model);

  useEffect(() => {
    thread.current?.scrollTo({ top: thread.current.scrollHeight, behavior: "smooth" });
  }, [turns.length]);

  const run = useCallback(
    async (text: string, style: string | null, as: Shape) => {
      if (!provider || !model) return;
      if (!provider.has_key) {
        setSettingsOpen(true);
        return;
      }
      const id = ++seq.current;
      setTurns((ts) => [
        ...ts,
        { id, idea: text, styleId: style, shape: as, where: `${provider.label} · ${model.label}`, status: "working", started: Date.now() },
      ]);
      try {
        const skin = await api.aiGenerate({
          provider: provider.id,
          model: model.id,
          idea: withStyle(text, style),
          shape: as,
          size: model.sizes[0] ?? null,
          reference_path: model.accepts_reference ? reference : null,
        });
        setTurns((ts) => ts.map((t) => (t.id === id ? { ...t, status: "done", skin } : t)));
        onGenerated(skin);
      } catch (e) {
        setTurns((ts) => ts.map((t) => (t.id === id ? { ...t, status: "error", error: errorMessage(e) } : t)));
      }
    },
    [provider, model, reference, onGenerated],
  );

  const send = (e?: FormEvent) => {
    e?.preventDefault();
    if (!canSend) return;
    // No key yet: keep what they typed and open the dialog that fixes it.
    if (!provider?.has_key) {
      setSettingsOpen(true);
      return;
    }
    const text = idea.trim();
    setIdea("");
    void run(text, styleId, shape);
  };

  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      send();
    }
  };

  const surprise = () => {
    const pick = SCENES[Math.floor(Math.random() * SCENES.length)];
    setIdea(pick);
    setStyleId(STYLES[Math.floor(Math.random() * STYLES.length)].id);
    input.current?.focus();
  };

  const pickReference = useCallback(async () => {
    if (!isTauri()) return setReference("/Users/you/Pictures/reference.jpg");
    const picked = await open({
      multiple: false,
      title: "Choose a reference picture",
      filters: [{ name: "Pictures", extensions: IMAGE_EXTENSIONS }],
    }).catch(() => null);
    if (typeof picked === "string") setReference(picked);
  }, []);

  if (loadError) {
    return (
      <section className="studio">
        <div className="empty">
          <span className="empty-glyph">
            <SparklesIcon size={22} />
          </span>
          <p className="empty-title">The assistant isn't in this build</p>
          <p className="empty-text">FolderSkin couldn't load its provider list: {loadError}. Rebuild the app, then open this again.</p>
          <button type="button" className="btn btn-secondary" onClick={load}>
            Try again
          </button>
        </div>
      </section>
    );
  }

  const placeholder = folderName ? `Describe a folder for ${folderName}…` : "Describe the folder you want…";

  return (
    <section className={hasThread ? "studio has-thread" : "studio"}>
      <div className="studio-thread" ref={thread}>
        {turns.map((t) => (
          <TurnCard
            key={t.id}
            turn={t}
            folderName={folderName}
            tryingOn={t.skin?.id === selectedId}
            onTryOn={(id) => {
              onTryOn(id);
              toast(folderName ? `Trying it on ${folderName}` : "Picked. Now drop a folder on the right", { tone: "info" });
            }}
            onAgain={() => void run(t.idea, t.styleId, t.shape)}
            onSettings={() => setSettingsOpen(true)}
            disabled={working}
          />
        ))}
      </div>

      <div className="studio-dock">
        {!hasThread && (
          <div className="studio-hero">
            <span className="studio-hero-glyph">
              <SparklesIcon size={22} playOnMount />
            </span>
            <h2 className="studio-title">What should your folder look like?</h2>
            <p className="studio-sub">
              Describe a scene and pick a style. It goes straight from your Mac to {provider?.label ?? "the provider"} with
              your own key, and comes back as a folder.
            </p>
          </div>
        )}

        <form className="composer" onSubmit={send}>
          <textarea
            ref={input}
            className="composer-input"
            rows={hasThread ? 1 : 2}
            value={idea}
            placeholder={placeholder}
            spellCheck
            onChange={(e) => setIdea(e.target.value)}
            onKeyDown={onKey}
          />
          {reference && (
            <div className="composer-ref">
              <PaperclipIcon size={13} />
              <span title={reference}>{baseName(reference)}</span>
              <button type="button" className="link-btn" onClick={() => setReference(null)}>
                Remove
              </button>
            </div>
          )}
          <div className="composer-bar">
            <div className="seg seg-sm" role="radiogroup" aria-label="what to make">
              {(
                [
                  ["folder", "Whole folder"],
                  ["skin", "Just the art"],
                ] as const
              ).map(([id, label]) => (
                <button
                  key={id}
                  type="button"
                  role="radio"
                  aria-checked={shape === id}
                  className={shape === id ? "seg-btn is-active" : "seg-btn"}
                  title={
                    id === "folder"
                      ? "The model paints the whole folder, like a poster. Shapes can vary a little."
                      : "The model paints flat art; FolderSkin wraps it onto its own folder."
                  }
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => setShape(id)}
                >
                  {label}
                </button>
              ))}
            </div>
            {model?.accepts_reference && (
              <button type="button" className="icon-btn" title="Add a reference picture" aria-label="add a reference picture" onClick={pickReference}>
                <PaperclipIcon />
              </button>
            )}
            <span className="composer-spacer" />
            <button type="button" className="model-pill" onClick={() => setSettingsOpen(true)} title="Provider, model and key">
              <span className={provider?.has_key ? "model-dot is-ready" : "model-dot"} aria-hidden="true" />
              <span className="model-pill-text">{provider ? `${provider.label} · ${model?.label ?? ""}` : "Choose a provider"}</span>
              <SlidersHorizontalIcon size={14} />
            </button>
            <button type="submit" className="send-btn" disabled={!canSend} aria-label="generate" title="Generate (Enter)">
              <ArrowUpIcon size={17} />
            </button>
          </div>
        </form>

        {!hasThread && (
          <>
            <div className="style-chips" role="radiogroup" aria-label="style">
              {STYLES.map((s) => (
                <button
                  key={s.id}
                  type="button"
                  role="radio"
                  aria-checked={styleId === s.id}
                  className={styleId === s.id ? "style-chip is-active" : "style-chip"}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => setStyleId((cur) => (cur === s.id ? null : s.id))}
                >
                  {s.label}
                </button>
              ))}
              <button type="button" className="style-chip is-surprise" onMouseDown={(e) => e.preventDefault()} onClick={surprise}>
                <SparklesIcon size={13} /> Surprise me
              </button>
            </div>
            <p className="studio-foot">
              {model && <span>{model.price_hint}, billed to your account.</span>}
              <button type="button" className="link-btn" onClick={() => setHelperOpen(true)}>
                No API key? Use Grok or ChatGPT's chat
              </button>
            </p>
          </>
        )}
        {hasThread && styleId && (
          <p className="studio-foot">
            Style: {STYLES.find((s) => s.id === styleId)?.label}
            <button type="button" className="link-btn" onClick={() => setStyleId(null)}>
              Clear
            </button>
          </p>
        )}
      </div>

      {settingsOpen && catalogue && (
        <StudioSettings
          catalogue={catalogue}
          providerId={providerId}
          modelId={model?.id ?? ""}
          onProvider={setProviderId}
          onModel={setModelId}
          onChanged={load}
          onClose={() => setSettingsOpen(false)}
          toast={toast}
        />
      )}
      {helperOpen && (
        <ChatHelper
          scene={idea}
          styleId={styleId}
          onImport={() => {
            setHelperOpen(false);
            onImport();
          }}
          onClose={() => setHelperOpen(false)}
          toast={toast}
        />
      )}
    </section>
  );
}

function TurnCard({
  turn,
  folderName,
  tryingOn,
  onTryOn,
  onAgain,
  onSettings,
  disabled,
}: {
  turn: Turn;
  folderName: string | null;
  tryingOn: boolean;
  onTryOn: (skinId: string) => void;
  onAgain: () => void;
  onSettings: () => void;
  disabled: boolean;
}) {
  const style = STYLES.find((s) => s.id === turn.styleId)?.label;
  return (
    <article className="turn">
      <p className="turn-ask">
        {turn.idea}
        {style && <span className="turn-style">{style}</span>}
      </p>
      {turn.status === "working" && <Developing shape={turn.shape} started={turn.started} where={turn.where} />}
      {turn.status === "done" && turn.skin && (
        <div className="turn-result">
          <img className="turn-img" src={turn.skin.thumbnail} alt="" draggable={false} />
          <div className="turn-meta">
            <p className="turn-name">{turn.skin.name}</p>
            <p className="turn-where">
              {turn.where} · saved to Yours
            </p>
            <div className="turn-actions">
              <button
                type="button"
                className={tryingOn ? "btn btn-primary is-done" : "btn btn-primary"}
                disabled={tryingOn}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => turn.skin && onTryOn(turn.skin.id)}
              >
                {tryingOn ? "Trying it on" : folderName ? `Try on ${folderName}` : "Try it on"}
              </button>
              <button type="button" className="btn btn-secondary" disabled={disabled} onMouseDown={(e) => e.preventDefault()} onClick={onAgain}>
                Make another
              </button>
            </div>
          </div>
        </div>
      )}
      {turn.status === "error" && (
        <div className="turn-error" role="alert">
          <p>{turn.error}</p>
          <div className="turn-actions">
            <button type="button" className="btn btn-secondary" disabled={disabled} onClick={onAgain}>
              Try again
            </button>
            <button type="button" className="btn btn-ghost" onClick={onSettings}>
              Check the key
            </button>
          </div>
        </div>
      )}
    </article>
  );
}

/** The folder "developing": a blueprint with light running through it, and honest progress. */
function Developing({ shape, started, where }: { shape: Shape; started: number; where: string }) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const t = window.setInterval(() => setNow(Date.now()), 500);
    return () => window.clearInterval(t);
  }, []);
  const seconds = Math.floor((now - started) / 1000);
  const steps = STEPS[shape];
  const step = steps[Math.min(steps.length - 1, Math.floor(seconds / 6))];
  return (
    <div className="turn-result is-developing" aria-live="polite">
      <div className="develop">
        <FolderGhost className="develop-ghost" />
        <span className="develop-light" aria-hidden="true" />
      </div>
      <div className="turn-meta">
        <p className="turn-name develop-step" key={step}>
          {step}…
        </p>
        <p className="turn-where">
          {where} · {seconds}s
        </p>
      </div>
    </div>
  );
}
