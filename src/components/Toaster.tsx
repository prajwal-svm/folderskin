import type { Toast } from "../hooks/useToasts";
import { OkBadge } from "./OkBadge";
import { SparklesIcon } from "./icons/sparkles";
import { branded } from "./Brand";

export function Toaster({ items, onDismiss }: { items: Toast[]; onDismiss: (id: number) => void }) {
  return (
    <div className="toasts" aria-live="polite">
      {items.map((t) => (
        <div key={t.id} className={t.leaving ? "toast is-leaving" : "toast"} role="status">
          <span className={t.tone === "ok" ? "toast-icon is-ok" : t.tone === "danger" ? "toast-icon is-danger" : "toast-icon"}>
            {t.tone === "danger" ? <span aria-hidden="true">!</span> : t.tone === "ok" ? <OkBadge size={18} playOnMount /> : <SparklesIcon size={15} playOnMount />}
          </span>
          <span className="toast-text">{branded(t.text)}</span>
          {t.action && (
            <button
              type="button"
              className="toast-action"
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => {
                t.action?.run();
                onDismiss(t.id);
              }}
            >
              {t.action.label}
            </button>
          )}
        </div>
      ))}
    </div>
  );
}
