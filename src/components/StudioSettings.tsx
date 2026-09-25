import type { AiCatalogue } from "../lib/tauri";
import type { ToastTone } from "../hooks/useToasts";
import { Modal } from "./Modal";
import { ProviderKeys } from "./ProviderKeys";
import { useT } from "../i18n";

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
  const t = useT();
  return (
    <Modal
      title={t("settings.ai.title")}
      sub={t("ai.keys.dialogSub")}
      onClose={onClose}
    >
      <ProviderKeys
        catalogue={catalogue}
        providerId={providerId}
        onProvider={onProvider}
        modelId={modelId}
        onModel={onModel}
        onChanged={onChanged}
        toast={toast}
      />
    </Modal>
  );
}
