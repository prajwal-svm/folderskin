import { buttonLabel, hasGlow, type State } from "../state/dropzone";
import { Brand } from "./Brand";

export function DropZone({
  state,
  browseLabel,
  thumbnail,
  onBrowse,
  onApply,
  onRevert,
}: {
  state: State;
  browseLabel: string;
  /** Preview of what the folder will look like: the selected skin, or the plain folder. */
  thumbnail: string | null;
  onBrowse: () => void;
  onApply: () => void;
  onRevert: () => void;
}) {
  const idle = state.phase === "idle";
  const busy = state.phase === "applying" || state.phase === "reverting";
  const label = buttonLabel(state);
  const cls = ["zone", idle ? "is-idle" : "has-folder", state.hover ? "is-hover" : "", hasGlow(state) ? "has-glow" : ""]
    .filter(Boolean)
    .join(" ");

  if (idle) {
    return (
      <section className={cls}>
        <button type="button" className="zone-hit" onClick={onBrowse} aria-label={`drag and drop a folder here, or click to browse ${browseLabel}`}>
          <p className="zone-title">drag &amp; drop a folder here</p>
          <p className="zone-sub">or click to browse {browseLabel}</p>
          <p className="zone-hint">
            or drop a photo to turn it into a <Brand />
          </p>
        </button>
        {state.error && <p className="zone-error" role="alert">{state.error}</p>}
      </section>
    );
  }

  return (
    <section className={cls} aria-live="polite">
      <div className="zone-card">
        {thumbnail && <img className="zone-preview" src={thumbnail} alt="" draggable={false} />}
        <p className="zone-name" title={state.folder?.path}>
          {state.folder?.name}
        </p>
        {state.phase === "folder" ? (
          <>
            <p className="zone-sub">
              pick a <span className="accent">skin</span> from the left
            </p>
            <p className="zone-hint">
              or drop a picture to make a <Brand />
            </p>
          </>
        ) : (
          <div className="zone-actions">
            <button
              type="button"
              className="btn-primary"
              disabled={busy || state.phase === "applied"}
              aria-busy={busy}
              onMouseDown={(e) => e.preventDefault()}
              onClick={state.phase === "ready" ? onApply : undefined}
            >
              {label}
            </button>
            {state.phase === "applied" && (
              <button
                type="button"
                className="btn-revert"
                title="put the default icon back"
                aria-label="put the default icon back"
                onMouseDown={(e) => e.preventDefault()}
                onClick={onRevert}
              >
                <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
                  <path d="M4 12a8 8 0 1 0 2.5-5.8" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" />
                  <path d="M3.5 3.5v5h5" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              </button>
            )}
          </div>
        )}
        {state.error && <p className="zone-error" role="alert">{state.error}</p>}
      </div>
    </section>
  );
}
