import { useCallback, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type AiCatalogue } from "../lib/tauri";
import { isTauri } from "../lib/devMock";
import type { ToastTone } from "../hooks/useToasts";
import { OkBadge } from "./OkBadge";
import { ProviderLogo } from "./ProviderLogo";
import { LocalSetup } from "./studio/LocalSetup";
import { CpuIcon } from "./icons/composer";
import { Select } from "./Select";
import { ExternalLinkIcon } from "./icons/external-link";
import { LoaderIcon } from "./icons/loader";

/**
 * The AI providers, the key for the chosen one, and optionally its model. Shared by the
 * studio's own dialog and Settings, so a key saved in one shows as saved in the other.
 */
export function ProviderKeys({
  catalogue,
  providerId,
  onProvider,
  modelId,
  onModel,
  onChanged,
  toast,
}: {
  catalogue: AiCatalogue;
  providerId: string;
  onProvider: (id: string) => void;
  /** With these, the chosen provider's model can be picked too, as the studio does. */
  modelId?: string;
  onModel?: (id: string) => void;
  /** Reload the catalogue after a key changes. */
  onChanged: () => void;
  toast: (text: string, opts?: { tone?: ToastTone }) => void;
}) {
  const provider = catalogue.providers.find((p) => p.id === providerId) ?? catalogue.providers[0];
  const model = provider?.models.find((m) => m.id === modelId) ?? provider?.models[0];
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<{ text: string; bad: boolean } | null>(null);

  const save = useCallback(async () => {
    if (!provider || !draft.trim()) return;
    setBusy(true);
    setNote(null);
    try {
      await api.aiSetKey(provider.id, draft.trim());
    } catch (e) {
      setNote({ text: errorMessage(e), bad: true });
      setBusy(false);
      return;
    }
    // It's saved whatever the check says next, so everything that shows keys hears of it now.
    setDraft("");
    onChanged();
    try {
      await api.aiTestKey(provider.id);
      toast(`${provider.label} accepted the key`, { tone: "ok" });
    } catch (e) {
      setNote({ text: `Saved, but ${provider.label} didn't accept it: ${errorMessage(e)}`, bad: true });
    } finally {
      setBusy(false);
    }
  }, [provider, draft, onChanged, toast]);

  const copy = useCallback(
    (text: string, what: string) => {
      navigator.clipboard
        .writeText(text)
        .then(() => toast(`${what} is copied`, { tone: "ok" }))
        .catch(() => toast("Couldn't copy it", { tone: "danger" }));
    },
    [toast],
  );

  const forget = useCallback(async () => {
    if (!provider) return;
    setBusy(true);
    try {
      await api.aiClearKey(provider.id);
      setNote({ text: `Removed the ${provider.label} key from this computer.`, bad: false });
      onChanged();
    } catch (e) {
      setNote({ text: errorMessage(e), bad: true });
    } finally {
      setBusy(false);
    }
  }, [provider, onChanged]);

  if (!provider) return null;
  const where = isTauri() ? "on this device" : "in this browser preview";

  return (
    <>
      <div className="provider-list" role="radiogroup" aria-label="provider">
        {catalogue.providers.map((p) => (
          <button
            key={p.id}
            type="button"
            role="radio"
            aria-checked={p.id === provider.id}
            className={p.id === provider.id ? "provider is-active" : "provider"}
            onClick={() => {
              onProvider(p.id);
              setNote(null);
              setDraft("");
            }}
          >
            <span className="provider-name">
              {p.kind === "local" ? <CpuIcon size={18} /> : <ProviderLogo id={p.id} size={18} />}
              {p.label}
            </span>
            {p.has_key ? (
              <OkBadge size={17} playOnMount label={p.kind === "local" ? "Set up" : "Key saved"} />
            ) : (
              <span className="provider-state">{p.kind === "local" ? "Free" : "No key"}</span>
            )}
          </button>
        ))}
      </div>

      {onModel && (
        <div className="field">
          <span className="field-label">Model</span>
          <Select
            label="model"
            className="is-field"
            value={model?.id ?? ""}
            onChange={onModel}
            options={provider.models.map((m) => ({ value: m.id, label: `${m.label} (${m.price_hint})` }))}
          />
          <span className="field-note">
            {provider.kind === "local"
              ? "Paints from your words, and from reference pictures."
              : model?.native_alpha
                ? "Returns a transparent background by itself."
                : "No transparency, so FolderSkin paints on a plain backdrop and cuts it out."}
          </span>
        </div>
      )}

      {provider.kind === "local" ? (
        <div className="field">
          <span className="field-label">On your machine</span>
          <LocalSetup onChanged={onChanged} copy={copy} />
        </div>
      ) : (
      <div className="field">
        <span className="field-label">{provider.label} API key</span>
        {provider.has_key ? (
          <div className="key-row">
            <span className="chip chip-ok">
              <OkBadge size={15} playOnMount /> Saved {where}
            </span>
            <button type="button" className="link-btn" disabled={busy} onClick={forget}>
              Remove key
            </button>
          </div>
        ) : (
          <div className="key-row">
            <input
              className="input"
              type="password"
              value={draft}
              placeholder={`Paste your key (${provider.key_hint})`}
              autoComplete="off"
              spellCheck={false}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void save()}
            />
            <button type="button" className="btn btn-primary" disabled={busy || !draft.trim()} onClick={save}>
              {busy ? <LoaderIcon /> : null}
              {busy ? "Checking" : "Save and check"}
            </button>
          </div>
        )}
        {note && <span className={note.bad ? "field-note is-bad" : "field-note"}>{note.text}</span>}
        <div className="key-links">
          <button type="button" className="link-btn" onClick={() => void openUrl(provider.keys_url).catch(() => {})}>
            Get a key <ExternalLinkIcon size={12} />
          </button>
          <button type="button" className="link-btn" onClick={() => void openUrl(provider.docs_url).catch(() => {})}>
            Pricing and docs <ExternalLinkIcon size={12} />
          </button>
        </div>
      </div>
      )}

      <p className="field-note">
        {provider.kind === "local"
          ? "Skins generated on this machine stay on it until you decide to share them with the community."
          : isTauri()
            ? "Your API keys are securely stored on this device."
            : "In this browser preview, keys last until you reload."}
      </p>
    </>
  );
}
