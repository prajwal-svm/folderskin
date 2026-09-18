import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type AiCatalogue, type AiProvider, type Skin } from "../lib/tauri";
import { isTauri } from "../lib/devMock";
import { CheckIcon } from "./icons/check";
import { ExternalLinkIcon } from "./icons/external-link";
import { ImageIcon } from "./icons/image";
import { LoaderIcon } from "./icons/loader";
import { SparklesIcon } from "./icons/sparkles";

type Phase = { kind: "idle" } | { kind: "working" } | { kind: "done"; skin: Skin } | { kind: "error"; message: string };

const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "heic", "heif"];

export function GenerateView({ onGenerated }: { onGenerated: (skin: Skin) => void }) {
  const [catalogue, setCatalogue] = useState<AiCatalogue | null>(null);
  const [providerId, setProviderId] = useState<string>("");
  const [modelId, setModelId] = useState<string>("");
  const [shape, setShape] = useState<"skin" | "folder">("skin");
  const [idea, setIdea] = useState("");
  const [reference, setReference] = useState<string | null>(null);
  const [keyDraft, setKeyDraft] = useState("");
  const [keyBusy, setKeyBusy] = useState(false);
  const [keyNote, setKeyNote] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });
  const [loadError, setLoadError] = useState<string | null>(null);

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
  const model = useMemo(
    () => provider?.models.find((m) => m.id === modelId) ?? provider?.models[0],
    [provider, modelId],
  );
  useEffect(() => {
    if (provider && !provider.models.some((m) => m.id === modelId)) setModelId(provider.models[0]?.id ?? "");
  }, [provider, modelId]);

  const saveKey = useCallback(async () => {
    if (!provider || !keyDraft.trim()) return;
    setKeyBusy(true);
    setKeyNote(null);
    try {
      await api.aiSetKey(provider.id, keyDraft.trim());
      await api.aiTestKey(provider.id);
      setKeyDraft("");
      setKeyNote(`Key saved to your ${isTauri() ? "system keychain" : "browser preview"} and accepted by ${provider.label}.`);
      load();
    } catch (e) {
      setKeyNote(errorMessage(e));
    } finally {
      setKeyBusy(false);
    }
  }, [provider, keyDraft, load]);

  const forgetKey = useCallback(async () => {
    if (!provider) return;
    setKeyBusy(true);
    try {
      await api.aiClearKey(provider.id);
      setKeyNote(`Key for ${provider.label} removed from the keychain.`);
      load();
    } catch (e) {
      setKeyNote(errorMessage(e));
    } finally {
      setKeyBusy(false);
    }
  }, [provider, load]);

  const pickReference = useCallback(async () => {
    if (!isTauri()) return setReference("/Users/you/Pictures/reference.jpg");
    const picked = await open({
      multiple: false,
      title: "Choose a reference picture",
      filters: [{ name: "Pictures", extensions: IMAGE_EXTENSIONS }],
    }).catch(() => null);
    if (typeof picked === "string") setReference(picked);
  }, []);

  const generate = useCallback(async () => {
    if (!provider || !model || !idea.trim()) return;
    setPhase({ kind: "working" });
    try {
      const skin = await api.aiGenerate({
        provider: provider.id,
        model: model.id,
        idea: idea.trim(),
        shape,
        size: model.sizes[0] ?? null,
        reference_path: model.accepts_reference ? reference : null,
      });
      setPhase({ kind: "done", skin });
      onGenerated(skin);
    } catch (e) {
      setPhase({ kind: "error", message: errorMessage(e) });
    }
  }, [provider, model, idea, shape, reference, onGenerated]);

  const working = phase.kind === "working";
  const ready = Boolean(provider?.has_key && idea.trim() && model && !working);

  return (
    <section className="view gen">
      <header className="view-head">
        <h2 className="view-title">
          Generate <span className="mark">with AI</span>
        </h2>
        <p className="view-sub">
          Your key, your account, your machine. FolderSkin has no server and sends nothing until you press Generate.
        </p>
      </header>

      {loadError ? (
        <div className="cards">
          <article className="card">
            <h3 className="card-title">The AI assistant is not available in this build</h3>
            <p className="card-text">
              FolderSkin could not reach its own provider list: {loadError}. This happens when the app is running a
              build made before the assistant existed. Rebuild the app, then open this view again.
            </p>
            <button type="button" className="btn btn-secondary" onMouseDown={(e) => e.preventDefault()} onClick={load}>
              Try again
            </button>
          </article>
        </div>
      ) : !catalogue ? (
        <p className="view-sub">Loading providers…</p>
      ) : (
        <div className="gen-grid">
          <div className="gen-main">
            <label className="field">
              <span className="field-label">What should it look like?</span>
              <textarea
                className="input textarea"
                rows={3}
                value={idea}
                placeholder="a night sky with green and violet aurora ribbons over dark mountains"
                onChange={(e) => setIdea(e.target.value)}
              />
            </label>

            <div className="presets">
              {catalogue.presets.map((p) => (
                <button key={p.id} type="button" className="preset" onMouseDown={(e) => e.preventDefault()} onClick={() => setIdea(p.idea)}>
                  {p.label}
                </button>
              ))}
            </div>

            <div className="field">
              <span className="field-label">What to draw</span>
              <div className="choice">
                <button
                  type="button"
                  className={shape === "skin" ? "choice-btn is-active" : "choice-btn"}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => setShape("skin")}
                >
                  <strong>Artwork</strong>
                  <span>Flat art that FolderSkin wraps onto its own folder. Lines up with the built-in skins.</span>
                </button>
                <button
                  type="button"
                  className={shape === "folder" ? "choice-btn is-active" : "choice-btn"}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => setShape("folder")}
                >
                  <strong>Whole folder</strong>
                  <span>The model draws the folder itself, so art can stand out in relief. Used as the icon directly.</span>
                </button>
              </div>
            </div>

            {model?.accepts_reference && (
              <div className="field">
                <span className="field-label">Reference picture (optional)</span>
                <div className="row">
                  <button type="button" className="btn btn-secondary" onMouseDown={(e) => e.preventDefault()} onClick={pickReference}>
                    <ImageIcon />
                    {reference ? "Change picture" : "Choose a picture"}
                  </button>
                  {reference && (
                    <>
                      <span className="ref-path">{reference}</span>
                      <button type="button" className="link-btn" onClick={() => setReference(null)}>
                        remove
                      </button>
                    </>
                  )}
                </div>
              </div>
            )}

            <div className="row gen-actions">
              <button type="button" className="btn btn-primary" disabled={!ready} aria-busy={working} onMouseDown={(e) => e.preventDefault()} onClick={generate}>
                <span className="btn-badge">{working ? <LoaderIcon /> : <SparklesIcon size={14} />}</span>
                {working ? "Generating…" : "Generate"}
              </button>
              {model && <span className="muted-note">{model.price_hint} · billed to your own account</span>}
            </div>

            {phase.kind === "error" && (
              <p className="zone-error" role="alert">
                {phase.message}
              </p>
            )}
            {phase.kind === "done" && (
              <div className="gen-result">
                <img src={phase.skin.thumbnail} alt="" />
                <div>
                  <p className="gen-result-title">
                    <CheckIcon playOnMount /> Added to your skins
                  </p>
                  <p className="view-sub">Pick it in the gallery, then drop a folder and apply.</p>
                </div>
              </div>
            )}
          </div>

          <aside className="gen-side">
            <div className="field">
              <span className="field-label">Provider</span>
              <select className="input" value={providerId} onChange={(e) => setProviderId(e.target.value)}>
                {catalogue.providers.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.label}
                    {p.has_key ? " ✓" : ""}
                  </option>
                ))}
              </select>
            </div>

            {provider && (
              <>
                <div className="field">
                  <span className="field-label">Model</span>
                  <select className="input" value={model?.id ?? ""} onChange={(e) => setModelId(e.target.value)}>
                    {provider.models.map((m) => (
                      <option key={m.id} value={m.id}>
                        {m.label}
                      </option>
                    ))}
                  </select>
                  <span className="field-note">
                    {model?.native_alpha
                      ? "Returns a transparent background directly."
                      : "No transparency: FolderSkin renders on a key colour and cuts it out."}
                  </span>
                </div>

                <div className="field">
                  <span className="field-label">API key</span>
                  {provider.has_key ? (
                    <div className="row">
                      <span className="chip">
                        <CheckIcon playOnMount /> Saved in your keychain
                      </span>
                      <button type="button" className="link-btn" disabled={keyBusy} onClick={forgetKey}>
                        remove
                      </button>
                    </div>
                  ) : (
                    <>
                      <input
                        className="input"
                        type="password"
                        value={keyDraft}
                        placeholder={provider.key_hint}
                        autoComplete="off"
                        spellCheck={false}
                        onChange={(e) => setKeyDraft(e.target.value)}
                      />
                      <button type="button" className="btn btn-secondary" disabled={keyBusy || !keyDraft.trim()} onMouseDown={(e) => e.preventDefault()} onClick={saveKey}>
                        {keyBusy ? <LoaderIcon /> : <CheckIcon />}
                        Save &amp; test
                      </button>
                    </>
                  )}
                  {keyNote && <span className="field-note">{keyNote}</span>}
                  <div className="row">
                    <button type="button" className="link-btn" onClick={() => void openUrl(provider.keys_url).catch(() => {})}>
                      Get a key <ExternalLinkIcon size={12} />
                    </button>
                    <button type="button" className="link-btn" onClick={() => void openUrl(provider.docs_url).catch(() => {})}>
                      Docs <ExternalLinkIcon size={12} />
                    </button>
                  </div>
                </div>

                <p className="field-note">
                  Keys are stored by your operating system's keychain, never in a FolderSkin file, and never leave your
                  machine except in the request you trigger.
                </p>
              </>
            )}
          </aside>
        </div>
      )}
    </section>
  );
}
