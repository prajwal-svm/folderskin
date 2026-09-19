import { useEffect, useRef, useState, type CSSProperties, type MouseEvent } from "react";
import type { Skin } from "../lib/tauri";
import { prettyPath } from "../lib/files";
import { fileBrowser } from "../lib/platform";
import type { State } from "../state/dropzone";
import { FolderGhost } from "./FolderGhost";
import { ArrowDownIcon } from "./icons/arrow-down";
import { ArrowLeftIcon } from "./icons/arrow-left";
import { FolderOpenIcon } from "./icons/folder-open";
import { LoaderIcon } from "./icons/loader";
import { RotateCcwIcon } from "./icons/rotate-ccw";
import { OkBadge } from "./OkBadge";

const SPARKS = Array.from({ length: 12 }, (_, k) => k);

/** How long the buttons that replace Apply ignore clicks: the second half of a double click. */
const SETTLE_MS = 450;

/** How long to wait for a folder's own icon before showing the plain folder in its place. */
const ICON_WAIT_MS = 600;

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
  customIcon,
  defaultThumb,
  os,
  browseLabel,
  onBrowse,
  onApply,
  onTryOn,
  onRevert,
  onReveal,
}: {
  state: State;
  /** The skin selected in the library, if any. */
  skin: Skin | null;
  /** The folder's real icon as the OS draws it now; undefined while it's on its way. */
  folderIcon: string | null | undefined;
  /** The folder wears an icon of its own (not the default one) that a revert would take off. */
  customIcon: boolean;
  defaultThumb: string | null;
  os: string;
  browseLabel: string;
  onBrowse: () => void;
  onApply: () => void;
  /** Puts the selected skin on a folder that's waiting with its own icon. */
  onTryOn: () => void;
  onRevert: () => void;
  onReveal: () => void;
}) {
  const { phase, folder, drag, error } = state;
  const busy = phase === "applying" || phase === "reverting";

  // The folder's own icon. Nothing shows while it loads, so a stand-in never swaps for it in view,
  // unless it's slow.
  const [slowFor, setSlowFor] = useState<string | null>(null);
  const waiting = folder !== null && folderIcon === undefined;
  useEffect(() => {
    if (!waiting || !folder) return;
    const t = window.setTimeout(() => setSlowFor(folder.path), ICON_WAIT_MS);
    return () => window.clearTimeout(t);
  }, [waiting, folder]);
  const ownIcon = folderIcon ?? (waiting && slowFor !== folder?.path ? null : defaultThumb);

  // What the folder shows: the icon it has, or the skin it is trying on. A folder that has just
  // replaced another shows its own icon for a moment first.
  const showsOwn = phase === "folder" || phase === "reverting" || (phase === "ready" && state.arriving);
  const src = phase === "idle" ? null : showsOwn ? ownIcon : (skin?.thumbnail ?? ownIcon);

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
          ) : folder && !drag ? null : skin && !drag ? (
            // A skin picked before any folder: shown as the folder it will make, with no outline.
            <img className="stage-ghost-skin" src={skin.thumbnail} alt="" key={skin.id} draggable={false} />
          ) : (
            // Nothing picked yet, or something dragged over: the empty folder with its outline.
            <>
              <FolderGhost className="stage-ghost" tone="mac" layer="fill" />
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
          skin={skin}
          customIcon={customIcon}
          fileBrowser={fileBrowser(os)}
          onApply={onApply}
          onTryOn={onTryOn}
          onRevert={onRevert}
          onReveal={onReveal}
          onBrowse={onBrowse}
        />

        <p className={error ? "stage-status is-error" : "stage-status"} role={error ? "alert" : undefined} aria-live="polite">
          {error ?? statusLine(state, skin, fileBrowser(os), customIcon)}
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

  // The one place that says the skin is on the folder; the buttons below only offer what's next.
  const eyebrow =
    phase === "applied" || (phase === "reverting" && state.appliedSkinId !== null) ? (
      <span className="chip chip-ok stage-eyebrow" key="applied">
        <OkBadge size={16} playOnMount /> Applied
      </span>
    ) : (phase === "ready" && !state.arriving) || phase === "applying" ? (
      <span className="chip chip-accent stage-eyebrow" key={`try:${skin?.id}`}>
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
  skin,
  customIcon,
  fileBrowser,
  onApply,
  onTryOn,
  onRevert,
  onReveal,
  onBrowse,
}: {
  state: State;
  skin: Skin | null;
  customIcon: boolean;
  fileBrowser: string;
  onApply: () => void;
  onTryOn: () => void;
  onRevert: () => void;
  onReveal: () => void;
  onBrowse: () => void;
}) {
  const { phase, drag } = state;
  if (drag || phase === "idle") return null;

  const noFocusSteal = (e: MouseEvent) => e.preventDefault();
  const nudge = (
    <p className="nudge">
      <span className="nudge-arrow">
        <ArrowLeftIcon />
      </span>
      Pick a skin to try it on
    </p>
  );

  // The folder as it is, wearing an icon of its own: it can be taken off, and a skin waiting to
  // go on is put on only when asked. "Removing" is taking that icon off, not one applied here.
  const removing = phase === "reverting" && state.appliedSkinId === null;
  const waiting = phase === "ready" && state.arriving && customIcon;
  if ((phase === "folder" && customIcon) || waiting || removing) {
    return (
      <div className="stage-actions">
        {skin ? (
          <button type="button" className="btn btn-primary btn-lg" disabled={removing} onMouseDown={noFocusSteal} onClick={onTryOn}>
            <ArrowDownIcon />
            <span className="stage-try">Try on {skin.name}</span>
          </button>
        ) : (
          nudge
        )}
        <button type="button" className="btn btn-ghost" disabled={removing} aria-busy={removing} onMouseDown={noFocusSteal} onClick={onRevert}>
          {removing ? <LoaderIcon size={15} /> : <RotateCcwIcon size={15} />}
          {removing ? "Removing…" : "Remove custom icon"}
        </button>
      </div>
    );
  }

  if (phase === "folder") return nudge;

  if (phase === "applied" || phase === "reverting") {
    return <DoneActions reverting={phase === "reverting"} fileBrowser={fileBrowser} onReveal={onReveal} onRevert={onRevert} />;
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

/**
 * After an apply: what to do next, in the same two places as Apply and "Choose a different
 * folder", so nothing jumps. Show in Finder sits where Apply was, so it waits out a double click.
 */
function DoneActions({
  reverting,
  fileBrowser,
  onReveal,
  onRevert,
}: {
  reverting: boolean;
  fileBrowser: string;
  onReveal: () => void;
  onRevert: () => void;
}) {
  const shownAt = useRef(Number.POSITIVE_INFINITY);
  useEffect(() => {
    shownAt.current = performance.now();
  }, []);
  const noFocusSteal = (e: MouseEvent) => e.preventDefault();

  return (
    <div className="stage-actions">
      <button
        type="button"
        className="btn btn-secondary btn-lg"
        disabled={reverting}
        onMouseDown={noFocusSteal}
        onClick={() => performance.now() - shownAt.current > SETTLE_MS && onReveal()}
      >
        <FolderOpenIcon size={16} />
        Show in {fileBrowser}
      </button>
      <button type="button" className="btn btn-ghost" disabled={reverting} aria-busy={reverting} onMouseDown={noFocusSteal} onClick={onRevert}>
        {reverting ? <LoaderIcon size={15} /> : <RotateCcwIcon size={15} />}
        {reverting ? "Reverting…" : "Revert"}
      </button>
    </div>
  );
}

/** One quiet line under the buttons: what just happened or what happens next. */
function statusLine(state: State, skin: Skin | null, fileBrowser: string, customIcon: boolean): string {
  switch (state.phase) {
    case "idle":
      return state.drag ? "" : "Pictures work too. Drop one to turn it into a skin.";
    case "folder":
      return customIcon ? "It has an icon of its own." : "";
    case "ready":
      return state.arriving && customIcon ? "It has an icon of its own." : "Nothing changes on disk until you apply.";
    case "applying":
      return `Writing ${skin?.name ?? "the skin"} into ${state.folder?.name}…`;
    case "applied":
      return `${fileBrowser} can take a second to catch up.`;
    case "reverting":
      return "Putting the default icon back…";
    default:
      return "";
  }
}
