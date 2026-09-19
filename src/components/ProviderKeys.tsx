import { useCallback, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type AiCatalogue } from "../lib/tauri";
import { isTauri } from "../lib/devMock";
import type { ToastTone } from "../hooks/useToasts";
import { OkBadge } from "./OkBadge";
import { ProviderLogo } from "./ProviderLogo";
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
      await api.aiTestKey(provider.id);
      setDraft("");
      toast(`${provider.label} accepted the key`, { tone: "ok" });
      onChanged();
    } catch (e) {
      setNote({ text: errorMessage(e), bad: true });
    } finally {
      setBusy(false);
    }
  }, [provider, draft, onChanged, toast]);

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
              <ProviderLogo id={p.id} size={18} />
              {p.label}
            </span>
            {p.has_key ? (
              <OkBadge size={17} playOnMount label="Key saved" />
            ) : (
              <span className="provider-state">No key</span>
            )}
          </button>
        ))}
      </div>

      {onModel && (
      <label className="field">
        <span className="field-label">Model</span>
        <select className="input" value={model?.id ?? ""} onChange={(e) => onModel(e.target.value)}>
          {provider.models.map((m) => (
            <option key={m.id} value={m.id}>
              {m.label} ({m.price_hint})
            </option>
          ))}
        </select>
        <span className="field-note">
          {model?.native_alpha
            ? "Returns a transparent background by itself."
            : "No transparency, so FolderSkin paints on magenta and cuts it out."}
        </span>
      </label>
      )}

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
              {busy ? "Checking…" : "Save and check"}
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

      <p className="field-note">
        {isTauri() ? "Your API keys are securely stored on this device." : "In this browser preview, keys last until you reload."}
      </p>
    </>
  );
}
