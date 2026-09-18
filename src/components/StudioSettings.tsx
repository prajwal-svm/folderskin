import { useCallback, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type AiCatalogue } from "../lib/tauri";
import { isTauri } from "../lib/devMock";
import type { ToastTone } from "../hooks/useToasts";
import { Modal } from "./Modal";
import { CheckIcon } from "./icons/check";
import { ExternalLinkIcon } from "./icons/external-link";
import { LoaderIcon } from "./icons/loader";

/** Provider, model and API key, in a dialog so the studio itself stays about the pictures. */
export function StudioSettings({
  catalogue,
  providerId,
  modelId,
  onProvider,
  onModel,
  onChanged,
  onClose,
  toast,
}: {
  catalogue: AiCatalogue;
  providerId: string;
  modelId: string;
  onProvider: (id: string) => void;
  onModel: (id: string) => void;
  /** Reload the catalogue after a key changes. */
  onChanged: () => void;
  onClose: () => void;
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
  const where = isTauri() ? "on this computer" : "in this browser preview";

  return (
    <Modal
      title="Provider and key"
      sub="FolderSkin has no server. Requests go from your computer to the provider you pick, billed to your account."
      onClose={onClose}
      footer={
        <button type="button" className="btn btn-primary" onClick={onClose}>
          Done
        </button>
      }
    >
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
            <span className="provider-name">{p.label}</span>
            <span className={p.has_key ? "provider-state is-ready" : "provider-state"}>{p.has_key ? "Key saved" : "No key"}</span>
          </button>
        ))}
      </div>

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

      <div className="field">
        <span className="field-label">{provider.label} API key</span>
        {provider.has_key ? (
          <div className="key-row">
            <span className="chip chip-ok">
              <CheckIcon size={13} playOnMount /> Saved {where}
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
        Your key is saved {where} in a file only your account can read, with no keychain prompts. It leaves your computer
        only inside the requests you start.
      </p>
    </Modal>
  );
}
