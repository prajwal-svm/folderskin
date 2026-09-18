import { useEffect, useRef, useState, type CSSProperties, type MouseEvent } from "react";
import type { Skin } from "../lib/tauri";
import { prettyPath } from "../lib/files";
import type { State } from "../state/dropzone";
import { FolderGhost } from "./FolderGhost";
import { ArrowDownIcon } from "./icons/arrow-down";
import { ArrowLeftIcon } from "./icons/arrow-left";
import { CheckIcon } from "./icons/check";
import { FolderOpenIcon } from "./icons/folder-open";
import { LoaderIcon } from "./icons/loader";
import { RotateCcwIcon } from "./icons/rotate-ccw";

const SPARKS = Array.from({ length: 12 }, (_, k) => k);

/** "Finder", "Explorer" or "Files": what people call the file browser on their OS. */
function fileBrowser(os: string): string {
  if (os === "macos") return "Finder";
  if (os === "windows") return "Explorer";
  return "Files";
}

/** CSS url() for a data URL or path, for masks that follow a folder's own shape. */
const maskOf = (src: string): CSSProperties => ({ maskImage: `url("${src}")`, WebkitMaskImage: `url("${src}")` });

/**
 * The folder island. Shows the folder the user picked (or an empty blueprint waiting for
 * one), tries the selected skin on it, and walks through apply and revert. The island itself
 * is the drop target and glows while something is dragged over the window.
 */
export function FolderStage({
  state,
  skin,
  folderIcon,
  defaultThumb,
  os,
  browseLabel,
  onBrowse,
  onApply,
  onRevert,
  onReveal,
}: {
  state: State;
  /** The skin selected in the library, if any. */
  skin: Skin | null;
  /** The folder's real icon as the OS draws it now. */
  folderIcon: string | null;
  defaultThumb: string | null;
  os: string;
  browseLabel: string;
  onBrowse: () => void;
  onApply: () => void;
  onRevert: () => void;
  onReveal: () => void;
}) {
  const { phase, folder, drag, error } = state;
  const busy = phase === "applying" || phase === "reverting";

  // What the folder shows: the icon it has, or the skin it is trying on.
  const src =
    phase === "folder" || phase === "reverting"
      ? (folderIcon ?? defaultThumb)
      : phase === "idle"
        ? null
        : (skin?.thumbnail ?? folderIcon ?? defaultThumb);

  const [shaking, setShaking] = useState(false);
  useEffect(() => {
    if (!error) return;
    setShaking(true);
    const t = window.setTimeout(() => setShaking(false), 440);
    return () => window.clearTimeout(t);
  }, [error]);

  const cls = [
    "island",
    "stage-island",
    `is-${phase}`,
    drag ? "is-drop-target" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <aside className={cls} aria-label="folder" data-tauri-drag-region>
      <span className="drop-glow" aria-hidden="true" />
      <div className="stage" data-tauri-drag-region>
        <button
          type="button"
          className={shaking ? "stage-art is-shaking" : "stage-art"}
          onClick={busy ? undefined : onBrowse}
          onMouseDown={(e) => e.preventDefault()}
          aria-label={folder ? `choose a different folder than ${folder.name}` : `choose a folder from ${browseLabel}`}
          title={folder ? "Choose a different folder" : undefined}
        >
          <span className="stage-halo" aria-hidden="true" />
          {src ? (
            <StageImage key={folder?.path ?? "none"} src={src} />
          ) : (
            <>
              <FolderGhost className="stage-ghost" tone="mac" layer="fill" />
              {skin && !drag && <img className="stage-ghost-skin" src={skin.thumbnail} alt="" key={skin.id} />}
              <FolderGhost className="stage-ghost" tone="mac" layer="line" />
            </>
          )}
          {busy && <span className="stage-ring" aria-hidden="true" />}
          {phase === "applied" && (
            <span className="stage-sparks" aria-hidden="true">
              {SPARKS.map((k) => (
                <i key={k} style={{ "--a": `${k * 30}deg`, "--k": k } as CSSProperties} />
              ))}
            </span>
          )}
        </button>

        <StageCopy state={state} skin={skin} browseLabel={browseLabel} />

        <StageActions
          state={state}
          fileBrowser={fileBrowser(os)}
          onApply={onApply}
          onRevert={onRevert}
          onReveal={onReveal}
          onBrowse={onBrowse}
        />

        <p className={error ? "stage-status is-error" : "stage-status"} role={error ? "alert" : undefined} aria-live="polite">
          {error ?? statusLine(state, skin, fileBrowser(os))}
        </p>
      </div>
    </aside>
  );
}

/**
 * The folder picture. The first picture after a folder arrives drops in; every later one
 * (a new skin tried on the same folder) squashes in with a sweep of light across it.
 */
function StageImage({ src }: { src: string }) {
  const shown = useRef(0);
  useEffect(() => {
    shown.current += 1;
  }, [src]);
  const swap = shown.current > 0;
  return (
    <>
      <img className={swap ? "stage-img is-swap" : "stage-img"} src={src} alt="" draggable={false} key={src} />
      {swap && <span className="stage-sweep" style={maskOf(src)} key={`sweep:${src}`} aria-hidden="true" />}
    </>
  );
}

function StageCopy({ state, skin, browseLabel }: { state: State; skin: Skin | null; browseLabel: string }) {
  const { phase, folder, drag } = state;

  if (drag) {
    const image = drag.kind === "image";
    return (
      <div className="stage-copy" key={`drag:${drag.kind}`}>
        <h2 className="stage-title">{image ? "Let go to add this picture" : "Let go to pick this folder"}</h2>
        <p className="stage-sub">
          {image
            ? `${drag.name || "It"} becomes a skin in Yours.`
            : drag.name
              ? `${drag.name} will show up here, ready for a skin.`
              : "It will show up here, ready for a skin."}
        </p>
      </div>
    );
  }

  if (!folder) {
    return (
      <div className="stage-copy" key={skin ? `waiting:${skin.id}` : "empty"}>
        <h2 className="stage-title">{skin ? "Now drop a folder" : "Drop a folder here"}</h2>
        <p className="stage-sub">
          {skin
            ? `${skin.name} is ready. Drop any folder here, or click to pick one from ${browseLabel}.`
            : `Or click to pick one from ${browseLabel}.`}
        </p>
      </div>
    );
  }

  const eyebrow =
    phase === "applied" ? (
      <span className="chip chip-ok stage-eyebrow" key="applied">
        <CheckIcon size={12} playOnMount /> Applied
      </span>
    ) : phase === "ready" || phase === "applying" ? (
      <span className="chip chip-sky stage-eyebrow" key={`try:${skin?.id}`}>
        Trying on {skin?.name}
      </span>
    ) : (
      <span className="chip stage-eyebrow" key="current">
        Current icon
      </span>
    );

  return (
    <div className="stage-copy" key={`folder:${folder.path}`}>
      {eyebrow}
      <h2 className="stage-title" title={folder.name}>
        {folder.name}
      </h2>
      <p className="stage-path" title={folder.path}>
        {prettyPath(folder.path)}
      </p>
    </div>
  );
}

function StageActions({
  state,
  fileBrowser,
  onApply,
  onRevert,
  onReveal,
  onBrowse,
}: {
  state: State;
  fileBrowser: string;
  onApply: () => void;
  onRevert: () => void;
  onReveal: () => void;
  onBrowse: () => void;
}) {
  const { phase, drag } = state;
  if (drag || phase === "idle") return null;

  if (phase === "folder") {
    return (
      <p className="nudge">
        <span className="nudge-arrow">
          <ArrowLeftIcon />
        </span>
        Pick a skin to try it on
      </p>
    );
  }

  const noFocusSteal = (e: MouseEvent) => e.preventDefault();

  if (phase === "applied" || phase === "reverting") {
    return (
      <div className="stage-actions">
        <button type="button" className="btn btn-primary btn-lg is-done" disabled onMouseDown={noFocusSteal}>
          {phase === "reverting" ? <LoaderIcon /> : <CheckIcon playOnMount />}
          {phase === "reverting" ? "Reverting…" : "Applied"}
        </button>
        <div className="row">
          <button type="button" className="btn btn-secondary" disabled={phase === "reverting"} onMouseDown={noFocusSteal} onClick={onReveal}>
            <FolderOpenIcon size={15} />
            Show in {fileBrowser}
          </button>
          <button type="button" className="btn btn-secondary" disabled={phase === "reverting"} onMouseDown={noFocusSteal} onClick={onRevert}>
            <RotateCcwIcon />
            Revert
          </button>
        </div>
      </div>
    );
  }

  const applying = phase === "applying";
  return (
    <div className="stage-actions">
      <button
        type="button"
        className="btn btn-primary btn-lg"
        disabled={applying}
        aria-busy={applying}
        onMouseDown={noFocusSteal}
        onClick={onApply}
      >
        {applying ? <LoaderIcon /> : <ArrowDownIcon />}
        {applying ? "Applying…" : "Apply skin"}
      </button>
      <button type="button" className="btn btn-ghost" disabled={applying} onMouseDown={noFocusSteal} onClick={onBrowse}>
        Choose a different folder
      </button>
    </div>
  );
}

/** One quiet line under the buttons: what just happened or what happens next. */
function statusLine(state: State, skin: Skin | null, fileBrowser: string): string {
  switch (state.phase) {
    case "idle":
      return state.drag ? "" : "Pictures work too. Drop one to turn it into a skin.";
    case "ready":
      return "Nothing changes on disk until you apply.";
    case "applying":
      return `Writing ${skin?.name ?? "the skin"} into ${state.folder?.name}…`;
    case "applied":
      return `Done. ${fileBrowser} can take a second to catch up.`;
    case "reverting":
      return "Putting the default icon back…";
    default:
      return "";
  }
}
