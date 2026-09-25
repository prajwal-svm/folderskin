import { useEffect, useRef, useState, type CSSProperties, type MouseEvent } from "react";
import type { Skin } from "../lib/tauri";
import { prettyPath } from "../lib/files";
import { osOf, type Os } from "../lib/platform";
import { t as tNow, useT } from "../i18n";
import { applyLabel, runSummary, tooMany } from "../lib/tree";
import type { State } from "../state/dropzone";
import { FolderGhost } from "./FolderGhost";
import { LookSwitch } from "./LookSwitch";
import { useLook } from "../state/look";
import type { FolderStyle } from "../composer/parts";
import { ArrowDownIcon } from "./icons/arrow-down";
import { ArrowLeftIcon } from "./icons/arrow-left";
import { FolderOpenIcon } from "./icons/folder-open";
import { LoaderIcon } from "./icons/loader";
import { RotateCcwIcon } from "./icons/rotate-ccw";
import { BadgeAlertIcon } from "./icons/badge-alert";
import { XIcon } from "./icons/composer";
import { OkBadge } from "./OkBadge";
import { clip } from "../lib/names";
import { explain } from "../lib/sentences";

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
  stopping,
  onIncludeSubfolders,
  onStop,
  onCarryOn,
  onTryAgain,
  onDismissRun,
  pickHint,
  onLook,
  onPutDown,
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
  /** Stop was pressed and the run is finishing the folder it's on. */
  stopping: boolean;
  onIncludeSubfolders: (on: boolean) => void;
  onStop: () => void;
  /** Carries on with the folders a stopped run didn't reach. */
  onCarryOn: () => void;
  /** Tries the folders a run couldn't change again. */
  onTryAgain: () => void;
  onDismissRun: () => void;
  /** What to do to see a skin on the folder, where there's none on it yet. */
  pickHint?: string;
  /** Chooses which folder skins go on, from the switch shown while there's no folder. */
  onLook?: (look: FolderStyle) => void;
  /** Puts down the skin shown before any folder, so the empty folder (and its switch) is back. */
  onPutDown?: () => void;
}) {
  const t = useT();
  const { phase, folder, drag, error } = state;
  const busy = phase === "applying" || phase === "reverting";
  const system = osOf(os);

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

  const tree = state.includeSubfolders || state.run !== null;
  const cls = [
    "island",
    "stage-island",
    `is-${phase}`,
    drag ? "is-drop-target" : "",
    tree && folder && !drag ? "has-tree" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <aside className={cls} aria-label={t("folder.stage.label")} data-tauri-drag-region>
      <span className="drop-glow" aria-hidden="true" />
      <div className="stage" data-tauri-drag-region>
        <button
          type="button"
          className={shaking ? "stage-art is-shaking" : "stage-art"}
          onClick={busy ? undefined : onBrowse}
          onMouseDown={(e) => e.preventDefault()}
          aria-label={folder ? t("folder.stage.chooseOtherLabel", { name: folder.name }) : t("folder.stage.chooseFromLabel", { place: browseLabel })}
          data-tip={folder ? t("folder.stage.chooseOther") : undefined}
        >
          <span className="stage-halo" aria-hidden="true" />
          {src && state.includeSubfolders && !drag && (
            // The folders inside, stacked behind it, while they're included.
            <span className="stage-stack" aria-hidden="true">
              <img className="stage-stack-img is-far" src={src} alt="" draggable={false} />
              <img className="stage-stack-img is-near" src={src} alt="" draggable={false} />
            </span>
          )}
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

        {/* Under the empty folder only: while a skin is previewed, the preview is what's shown,
            with the way back to the empty folder (Escape does the same). */}
        {!folder && !skin && !drag && onLook && <StageLook onLook={onLook} />}
        {!folder && skin && !drag && onPutDown && (
          <button type="button" className="link-btn stage-put-down" data-tip={t("folder.stage.backTip")} onMouseDown={(e) => e.preventDefault()} onClick={onPutDown}>
            {t("folder.stage.back")}
          </button>
        )}

        {folder && !drag && phase !== "idle" && <SubfolderSwitch state={state} onChange={onIncludeSubfolders} />}

        {state.run && !busy && !drag && (
          <RunResult state={state} skinName={skin?.name ?? null} onCarryOn={onCarryOn} onTryAgain={onTryAgain} onDismiss={onDismissRun} />
        )}

        {busy && state.progress ? (
          <RunProgress state={state} stopping={stopping} onStop={onStop} />
        ) : (
          <StageActions
            state={state}
            skin={skin}
            customIcon={customIcon}
            os={system}
            onApply={onApply}
            onTryOn={onTryOn}
            onRevert={onRevert}
            onReveal={onReveal}
            onBrowse={onBrowse}
            pickHint={pickHint ?? t("folder.stage.pickHint")}
          />
        )}

        <p className={error ? "stage-status is-error" : "stage-status"} role={error ? "alert" : undefined} aria-live="polite">
          {error ?? statusLine(state, skin, system, customIcon, stopping)}
        </p>
      </div>
    </aside>
  );
}

/** Which folder skins go on, chosen while there's no folder and no skin yet: the empty one above shows it. */
function StageLook({ onLook }: { onLook: (look: FolderStyle) => void }) {
  const t = useT();
  return <LookSwitch className="stage-look" label={t("folder.look.appLabel")} value={useLook()} onChange={onLook} />;
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
  const t = useT();
  const { phase, folder, drag } = state;

  if (drag) {
    const image = drag.kind === "image";
    return (
      <div className="stage-copy" key={`drag:${drag.kind}`}>
        <h2 className="stage-title is-words">{image ? t("folder.stage.drag.imageTitle") : t("folder.stage.drag.folderTitle")}</h2>
        <p className="stage-sub">
          {image
            ? drag.name
              ? t("folder.stage.drag.imageNamed", { name: clip(drag.name) })
              : t("folder.stage.drag.image")
            : drag.name
              ? t("folder.stage.drag.folderNamed", { name: clip(drag.name) })
              : t("folder.stage.drag.folder")}
        </p>
      </div>
    );
  }

  if (!folder) {
    return (
      <div className="stage-copy" key={skin ? `waiting:${skin.id}` : "empty"}>
        <h2 className="stage-title is-words">{skin ? t("folder.stage.empty.titleWithSkin") : t("folder.stage.empty.title")}</h2>
        <p className="stage-sub">
          {skin
            ? t("folder.stage.empty.subWithSkin", { name: clip(skin.name), place: browseLabel })
            : t("folder.stage.empty.sub", { place: browseLabel })}
        </p>
      </div>
    );
  }

  // The one place that says the skin is on the folder; the buttons below only offer what's next.
  const eyebrow =
    phase === "applied" || (phase === "reverting" && state.appliedSkinId !== null) ? (
      <span className="chip chip-ok stage-eyebrow" key="applied">
        <OkBadge size={16} playOnMount /> {t("folder.stage.applied")}
      </span>
    ) : (phase === "ready" && !state.arriving) || phase === "applying" ? (
      <span className="chip chip-accent stage-eyebrow" key={`try:${skin?.id}`} data-tip={skin?.name} data-tip-overflow>
        <span className="chip-text">{t("folder.stage.tryingOn", { name: skin?.name ?? "" })}</span>
      </span>
    ) : (
      <span className="chip stage-eyebrow" key="current">
        {t("folder.stage.currentIcon")}
      </span>
    );

  return (
    <div className="stage-copy" key={`folder:${folder.path}`}>
      {eyebrow}
      <h2 className="stage-title" data-tip={folder.name} data-tip-overflow>
        {folder.name}
      </h2>
      <p className="stage-path" data-tip={folder.path} data-tip-overflow>
        {prettyPath(folder.path)}
      </p>
    </div>
  );
}

function StageActions({
  state,
  skin,
  customIcon,
  os,
  onApply,
  onTryOn,
  onRevert,
  onReveal,
  onBrowse,
  pickHint,
}: {
  state: State;
  skin: Skin | null;
  customIcon: boolean;
  os: Os;
  onApply: () => void;
  onTryOn: () => void;
  onRevert: () => void;
  onReveal: () => void;
  onBrowse: () => void;
  pickHint: string;
}) {
  const t = useT();
  const { phase, drag } = state;
  if (drag || phase === "idle") return null;

  const noFocusSteal = (e: MouseEvent) => e.preventDefault();
  const inside = state.includeSubfolders && state.subfolders ? state.subfolders.count : 0;
  // An apply over the tree has run: finished (some may have failed), or stopped, which its
  // summary offers to carry on. Either way what's next is showing or reverting it.
  const treeDone = state.run?.kind === "apply";
  const nudge = (
    <p className="nudge">
      <span className="nudge-arrow">
        <ArrowLeftIcon />
      </span>
      {pickHint}
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
            <span className="stage-try">{t("folder.stage.tryOn", { name: skin.name })}</span>
          </button>
        ) : (
          nudge
        )}
        <button type="button" className="btn btn-ghost" disabled={removing} aria-busy={removing} onMouseDown={noFocusSteal} onClick={onRevert}>
          {removing ? <LoaderIcon size={15} /> : <RotateCcwIcon size={15} />}
          {removing ? t("folder.stage.removing") : inside ? t("folder.stage.removeIcons") : t("folder.stage.removeIcon")}
        </button>
      </div>
    );
  }

  if (phase === "folder") return nudge;

  // Applied to the folder itself, but the folders inside it are now included too: offer those.
  const moreToDo = phase === "applied" && inside > 0 && !treeDone;
  if ((phase === "applied" || phase === "reverting") && !moreToDo) {
    const reach = inside > 0 && state.run?.kind === "apply" ? state.run.changed.length : 0;
    return (
      <DoneActions
        reverting={phase === "reverting"}
        revertLabel={
          reach > 1 ? (state.run?.stopped ? t("folder.stage.revertThese", { count: reach }) : t("folder.stage.revertAll", { count: reach })) : t("folder.stage.revert")
        }
        os={os}
        onReveal={onReveal}
        onRevert={onRevert}
      />
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
        {applying ? t("folder.stage.applying") : inside ? applyLabel(inside) : t("folder.stage.applySkin")}
      </button>
      <button type="button" className="btn btn-ghost" disabled={applying} onMouseDown={noFocusSteal} onClick={onBrowse}>
        {t("folder.stage.chooseOther")}
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
  revertLabel,
  os,
  onReveal,
  onRevert,
}: {
  reverting: boolean;
  revertLabel: string;
  os: Os;
  onReveal: () => void;
  onRevert: () => void;
}) {
  const t = useT();
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
        {t(`common.showIn.${os}`)}
      </button>
      <button type="button" className="btn btn-ghost" disabled={reverting} aria-busy={reverting} onMouseDown={noFocusSteal} onClick={onRevert}>
        {reverting ? <LoaderIcon size={15} /> : <RotateCcwIcon size={15} />}
        {reverting ? t("folder.stage.reverting") : revertLabel}
      </button>
    </div>
  );
}

/**
 * "Include subfolders": the folder and every folder inside it get the skin, or lose their icons,
 * together. It shows only for a folder with folders inside, says how many, and can't be used on
 * more than a run can take.
 */
function SubfolderSwitch({ state, onChange }: { state: State; onChange: (on: boolean) => void }) {
  const t = useT();
  const s = state.subfolders;
  if (!s || s.count === 0 || !state.folder) return null;
  const on = state.includeSubfolders;
  const busy = state.phase === "applying" || state.phase === "reverting";
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      disabled={busy || s.more}
      className={on ? "stage-scope is-on" : "stage-scope"}
      onMouseDown={(e) => e.preventDefault()}
      onClick={() => onChange(!on)}
    >
      <span className="stage-scope-text">
        <span className="stage-scope-title">{t("folder.stage.includeSubfolders")}</span>
        <span className="stage-scope-sub">{s.more ? tooMany("folders") : t("folder.stage.inside", { count: s.count })}</span>
      </span>
      <span className={on ? "switch is-on" : "switch"} aria-hidden="true">
        <span className="knob" />
      </span>
    </button>
  );
}

/** A run over the folder and its subfolders, under way: how far, which folder, and Stop. */
function RunProgress({ state, stopping, onStop }: { state: State; stopping: boolean; onStop: () => void }) {
  const t = useT();
  const p = state.progress;
  if (!p) return null;
  const share = p.total > 0 ? Math.min(100, (p.done / p.total) * 100) : 0;
  const verb = state.phase === "applying" ? t("folder.stage.applying") : t("folder.stage.reverting");
  return (
    <div className="stage-actions stage-progress" role="status" aria-live="polite">
      <p className="stage-progress-line">
        <span className="stage-progress-title">{stopping ? t("folder.stage.stopping") : verb}</span>
        <span className="stage-progress-count">{t("folder.stage.doneOf", { done: p.done, total: p.total })}</span>
      </p>
      <span className="stage-progress-bar" aria-hidden="true">
        <span style={{ width: `${share}%` }} />
      </span>
      <p className="stage-progress-name" data-tip={p.name} data-tip-overflow>
        {p.done === 0 ? t("folder.stage.startingWith", { name: clip(p.name) }) : p.name}
      </p>
      <button type="button" className="btn btn-ghost" disabled={stopping} onMouseDown={(e) => e.preventDefault()} onClick={onStop}>
        {stopping ? <LoaderIcon size={15} /> : null}
        {stopping ? t("folder.stage.finishingThis") : t("folder.stage.stop")}
      </button>
    </div>
  );
}

/**
 * What the last run over the tree did: all of it, some with the ones that failed listed (and a
 * way to try them again), or where it stopped (and a way to carry on).
 */
function RunResult({
  state,
  skinName,
  onCarryOn,
  onTryAgain,
  onDismiss,
}: {
  state: State;
  skinName: string | null;
  onCarryOn: () => void;
  onTryAgain: () => void;
  onDismiss: () => void;
}) {
  const t = useT();
  const run = state.run;
  if (!run) return null;
  const { title, detail, tone } = runSummary(run, skinName);
  const failed = run.failed;
  // Stopped before anything changed, the Apply button below does the same.
  const carryOn = run.stopped && run.remaining.length > 0 && run.changed.length > 0;
  return (
    <div className={`stage-result is-${tone}`} role="status">
      <div className="stage-result-head">
        <span className="stage-result-icon" aria-hidden="true">
          {tone === "ok" ? <OkBadge size={18} playOnMount /> : <BadgeAlertIcon size={17} />}
        </span>
        <p className="stage-result-title">{title}</p>
        <button type="button" className="stage-result-x" aria-label={t("folder.stage.hideSummaryLabel")} data-tip={t("folder.stage.hideSummary")} onClick={onDismiss}>
          <XIcon size={13} />
        </button>
      </div>
      {detail && <p className="stage-result-detail">{detail}</p>}
      {failed.length > 0 && (
        <details className="stage-result-failed">
          <summary>{t("folder.stage.which", { count: failed.length })}</summary>
          <ul>
            {failed.map((f) => (
              <li key={f.path} data-tip={f.path} data-tip-overflow>
                <span className="stage-result-name">{f.name}</span>
                <span className="stage-result-reason">{explain(f.reason)}</span>
              </li>
            ))}
          </ul>
        </details>
      )}
      {(carryOn || (!run.stopped && failed.length > 0)) && (
        <div className="stage-result-actions">
          {carryOn && (
            <button type="button" className="chip-btn" onClick={onCarryOn}>
              {t("folder.stage.carryOnWith", { count: run.remaining.length })}
            </button>
          )}
          {!run.stopped && failed.length > 0 && (
            <button type="button" className="chip-btn" onClick={onTryAgain}>
              <RotateCcwIcon size={13} />
              {t("folder.stage.tryAgain", { count: failed.length })}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

/** One quiet line under the buttons: what just happened or what happens next. */
function statusLine(state: State, skin: Skin | null, os: Os, customIcon: boolean, stopping: boolean): string {
  const t = tNow;
  const inside = state.includeSubfolders && state.subfolders ? state.subfolders.count : 0;
  if (state.progress && (state.phase === "applying" || state.phase === "reverting")) {
    if (stopping) return t("folder.stage.status.doneStay");
    return state.phase === "applying" ? t(`folder.stage.status.catchesUp.${os}`) : t("folder.stage.status.puttingIconsBack");
  }
  if (inside > 0 && (state.phase === "ready" || (state.phase === "applied" && state.run === null))) {
    return t("folder.stage.status.skipped");
  }
  switch (state.phase) {
    case "idle":
      return state.drag ? "" : t("folder.stage.status.picturesToo");
    case "folder":
      return customIcon ? t("folder.stage.status.ownIcon") : "";
    case "ready":
      return state.arriving && customIcon ? t("folder.stage.status.ownIcon") : t("folder.stage.status.nothingChanges");
    case "applying":
      return t("folder.stage.status.writing", {
        skin: skin ? clip(skin.name) : t("folder.run.theSkin"),
        folder: state.folder ? clip(state.folder.name) : t("folder.stage.theFolder"),
      });
    case "applied":
      return t(`folder.stage.status.catchUp.${os}`);
    case "reverting":
      return t("folder.stage.status.puttingIconBack");
    default:
      return "";
  }
}
