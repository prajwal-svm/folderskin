import { buttonLabel, hasGlow, type State } from "../state/dropzone";
import { Handles } from "./FolderThumb";
import { IconArrowDown, IconCheck, IconFolder, IconSpinner, IconUndo } from "./icons";

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
  /** What the folder looks like now, or will look like with the chosen skin. */
  thumbnail: string | null;
  onBrowse: () => void;
  onApply: () => void;
  onRevert: () => void;
}) {
  const idle = state.phase === "idle";
  const busy = state.phase === "applying" || state.phase === "reverting";
  const label = buttonLabel(state);
  const selected = hasGlow(state);
  const cls = [
    "zone",
    idle ? "is-idle" : "has-folder",
    state.hover ? "is-hover" : "",
    selected ? "sel" : "",
    busy ? "is-busy" : "",
    state.phase === "applied" ? "is-done" : "",
  ]
    .filter(Boolean)
    .join(" ");

  if (idle) {
    return (
      <section className={cls}>
        <svg className="ants" aria-hidden="true">
          <rect x="1" y="1" rx="19" ry="19" />
        </svg>
        <button
          type="button"
          className="zone-hit"
          onClick={onBrowse}
          aria-label={`drag and drop a folder here, or click to browse ${browseLabel}`}
        >
          <span className="zone-glyph">
            <IconFolder />
          </span>
          <p className="zone-title">Drag &amp; drop a folder here</p>
          <p className="zone-sub">or click to browse {browseLabel}</p>
          <p className="zone-sub zone-hint">Drop a photo instead to make a skin from it</p>
        </button>
        {state.error && (
          <p className="zone-error" role="alert">
            {state.error}
          </p>
        )}
      </section>
    );
  }

  const badge = state.phase === "applied" ? <IconCheck /> : busy ? <IconSpinner /> : <IconArrowDown />;

  return (
    <section className={cls} aria-live="polite">
      {selected && <Handles />}
      <div className="zone-card" key={state.folder?.path}>
        {thumbnail && <img className="zone-preview" src={thumbnail} alt="" draggable={false} key={thumbnail} />}
        <p className="zone-name" title={state.folder?.path}>
          {state.folder?.name}
        </p>
        <p className="zone-path">{state.folder?.path}</p>
        {state.phase === "folder" ? (
          <p className="zone-sub zone-hint">Pick a skin from the left, or drop a photo</p>
        ) : (
          <div className="zone-actions">
            <button
              type="button"
              className={state.phase === "applied" ? "btn btn-primary is-done" : "btn btn-primary"}
              disabled={busy || state.phase === "applied"}
              aria-busy={busy}
              onMouseDown={(e) => e.preventDefault()}
              onClick={state.phase === "ready" ? onApply : undefined}
            >
              <span className="btn-badge" key={state.phase}>
                {badge}
              </span>
              {label}
            </button>
            {state.phase === "applied" && (
              <button
                type="button"
                className="btn btn-secondary"
                title="put the default icon back"
                onMouseDown={(e) => e.preventDefault()}
                onClick={onRevert}
              >
                <IconUndo />
                Revert
              </button>
            )}
          </div>
        )}
        {state.error && (
          <p className="zone-error" role="alert">
            {state.error}
          </p>
        )}
      </div>
    </section>
  );
}
