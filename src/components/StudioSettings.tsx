import type { AiCatalogue } from "../lib/tauri";
import type { ToastTone } from "../hooks/useToasts";
import { Modal } from "./Modal";
import { ProviderKeys } from "./ProviderKeys";

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
