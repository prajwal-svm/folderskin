import { useCallback, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type AiCatalogue } from "../lib/tauri";
import { isTauri } from "../lib/devMock";
import type { ToastTone } from "../hooks/useToasts";
import { OkBadge } from "./OkBadge";
import { ProviderLogo } from "./ProviderLogo";
import { LocalSetup } from "./studio/LocalSetup";
import { useLocalSetupRunning } from "../state/localSetupRun";
import { CpuIcon } from "./icons/composer";
import { Select } from "./Select";
import { ExternalLinkIcon } from "./icons/external-link";
import { LoaderIcon } from "./icons/loader";
import { branded } from "./Brand";
import { t as tNow, useT } from "../i18n";
import { explain } from "../lib/sentences";
import { providerName } from "../lib/providerNames";

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
  const t = useT();
  const provider = catalogue.providers.find((p) => p.id === providerId) ?? catalogue.providers[0];
  const model = provider?.models.find((m) => m.id === modelId) ?? provider?.models[0];
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<{ text: string; bad: boolean } | null>(null);
  const settingUp = useLocalSetupRunning();

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
      toast(tNow("ai.keys.accepted", { provider: provider.label }), { tone: "ok" });
    } catch (e) {
      setNote({ text: tNow("ai.keys.notAccepted", { provider: provider.label, reason: errorMessage(e) }), bad: true });
    } finally {
      setBusy(false);
    }
  }, [provider, draft, onChanged, toast]);

  const copy = useCallback(
    (text: string) => {
      navigator.clipboard
        .writeText(text)
        .then(() => toast(tNow("ai.studio.questionCopied"), { tone: "ok" }))
        .catch(() => toast(tNow("ai.studio.copyFailed"), { tone: "danger" }));
    },
    [toast],
  );

  const forget = useCallback(async () => {
    if (!provider) return;
    setBusy(true);
    try {
      await api.aiClearKey(provider.id);
      setNote({ text: tNow("ai.keys.removed", { provider: provider.label }), bad: false });
      onChanged();
    } catch (e) {
      setNote({ text: errorMessage(e), bad: true });
    } finally {
      setBusy(false);
    }
  }, [provider, onChanged]);

  if (!provider) return null;


  return (
    <>
      <div className="provider-list" role="radiogroup" aria-label={t("ai.keys.providerLabel")}>
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
              {providerName(p.label)}
            </span>
            {p.kind === "local" && settingUp ? (
              // Downloading, whichever provider is shown below.
              <span className="provider-state provider-busy" role="img" aria-label={t("ai.keys.downloadingLabel")} data-tip={t("ai.keys.downloadingTip")}>
                <LoaderIcon size={16} />
              </span>
            ) : p.has_key ? (
              <OkBadge size={17} playOnMount label={p.kind === "local" ? t("ai.keys.setUp") : t("ai.prompt.status.keySaved")} />
            ) : (
              <span className="provider-state">{p.kind === "local" ? t("ai.keys.notSetUp") : t("ai.keys.noKey")}</span>
            )}
          </button>
        ))}
      </div>

      {/* The Local Model has one model, which its own panel below names. */}
      {onModel && provider.kind !== "local" && (
        <div className="field">
          <span className="field-label">{t("ai.keys.model")}</span>
          <Select
            label={t("ai.keys.modelLabel")}
            className="is-field"
            value={model?.id ?? ""}
            onChange={onModel}
            options={provider.models.map((m) => ({ value: m.id, label: t("ai.keys.modelOption", { model: m.label, price: explain(m.price_hint) }) }))}
          />
          <span className="field-note">{branded(model?.native_alpha ? t("ai.keys.alpha") : t("ai.keys.noAlpha"))}</span>
        </div>
      )}

      {provider.kind === "local" ? (
        <div className="field">
          <span className="field-label">{t("ai.keys.onYourMachine")}</span>
          <LocalSetup onChanged={onChanged} copy={copy} />
        </div>
      ) : (
      <div className="field">
        <span className="field-label">{t("ai.keys.keyLabel", { provider: provider.label })}</span>
        {provider.has_key ? (
          <div className="key-row">
            <span className="chip chip-ok">
              <OkBadge size={15} playOnMount /> {isTauri() ? t("ai.keys.savedHere") : t("ai.keys.savedPreview")}
            </span>
            <button type="button" className="link-btn" disabled={busy} onClick={forget}>
              {t("ai.keys.remove")}
            </button>
          </div>
        ) : (
          <div className="key-row">
            <input
              className="input"
              type="password"
              value={draft}
              placeholder={t("ai.keys.paste", { hint: explain(provider.key_hint) })}
              autoComplete="off"
              spellCheck={false}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void save()}
            />
            <button type="button" className="btn btn-primary" disabled={busy || !draft.trim()} onClick={save}>
              {busy ? <LoaderIcon /> : null}
              {busy ? t("ai.keys.checking") : t("ai.keys.save")}
            </button>
          </div>
        )}
        {note && <span className={note.bad ? "field-note is-bad" : "field-note"}>{branded(note.text)}</span>}
        <div className="key-links">
          <button type="button" className="link-btn" onClick={() => void openUrl(provider.keys_url).catch(() => {})}>
            {t("ai.keys.getKey")} <ExternalLinkIcon size={12} />
          </button>
          <button type="button" className="link-btn" onClick={() => void openUrl(provider.docs_url).catch(() => {})}>
            {t("ai.keys.pricing")} <ExternalLinkIcon size={12} />
          </button>
        </div>
      </div>
      )}

      <p className="field-note">
        {provider.kind === "local" ? t("ai.keys.localNote") : isTauri() ? t("ai.keys.storedNote") : t("ai.keys.previewNote")}
      </p>
    </>
  );
}
