import { useEffect, useState, type CSSProperties, type MouseEvent, type ReactNode } from "react";
import { useT } from "../i18n";
import { both, canCarryOn, canRetry, canUndo, doneOf, runDoing, runSummary, shareOf, type TreeRun } from "../lib/tree";
import { clip } from "../lib/names";
import { treeRuns } from "../state/treeRun";
import { OkBadge } from "./OkBadge";
import { BadgeAlertIcon } from "./icons/badge-alert";
import { FolderOpenIcon } from "./icons/folder-open";
import { LoaderIcon } from "./icons/loader";
import { RotateCcwIcon } from "./icons/rotate-ccw";
import { XIcon } from "./icons/composer";

/** The skin a run puts on, as the library has it. */
type DockSkin = { name: string; thumbnail: string } | null;

/**
 * A window shorter than the one FolderSkin opens in, where the dock at its full size would push
 * the end of the sidebar's list out of view.
 */
const SHORT = "(max-height: 719px)";

function useShortWindow(): boolean {
  const [short, setShort] = useState(() => typeof matchMedia === "function" && matchMedia(SHORT).matches);
  useEffect(() => {
    if (typeof matchMedia !== "function") return;
    const mq = matchMedia(SHORT);
    const onChange = () => setShort(mq.matches);
    onChange();
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);
  return short;
}

/**
 * The run over a folder's tree, from any view: at the foot of the sidebar while a run is going,
 * with its skin, what it's doing and to which folder, how far it has got and Stop, and once it
 * has ended, how it went and what the folder panel offers after a run (carry on, try again, undo)
 * until it's put away. A click on it brings that folder back to the folder panel. In a short
 * window it's one row: the skin, what it's doing and how far (or how it went, and where), and
 * Stop, the rest a click away in the folder panel. Folded to a rail, it's the skin in a ring that
 * fills as the run goes, and says the rest in a tooltip.
 */
export function RunDock({
  run,
  skin,
  folderIcon,
  rail,
  onShow,
  onStop,
  onCarryOn,
  onRetry,
  onUndo,
  onDismiss,
}: {
  run: TreeRun;
  skin: DockSkin;
  /** The plain folder, for a run that puts no skin on, or whose skin has gone. */
  folderIcon: string | null;
  rail: boolean;
  onShow: () => void;
  onStop: () => void;
  onCarryOn: () => void;
  onRetry: () => void;
  onUndo: () => void;
  onDismiss: () => void;
}) {
  const t = useT();
  const short = useShortWindow();
  const going = run.running;
  const skinName = skin?.name ?? null;
  const what = going ? doing(t, run, skinName) : runSummary(run, skinName).title;
  const summary = going ? null : runSummary(run, skinName);
  // How it went says the rest in the title's tooltip: the folder panel has room for it.
  const whole = summary?.detail ? both(what, summary.detail) : what;
  const expected = treeRuns.expected(run.id);
  const progress = doneOf(run, expected);
  const share = shareOf(run, expected);
  // The skin a run puts on or takes off; taking every custom icon off shows the plain folder.
  const picture = run.kind === "apply" || run.undoing ? (skin?.thumbnail ?? folderIcon) : folderIcon;
  const keep = (e: MouseEvent) => e.preventDefault();
  const tone = going ? "is-going" : summary?.tone === "ok" ? "is-ok" : "is-warn";

  if (rail) {
    const tip = going ? t("folder.dock.tip", { what, folder: clip(run.name), progress }) : whole;
    return (
      <button
        type="button"
        className={`dock-rail ${tone}`}
        aria-label={tip}
        data-tip={tip}
        data-tip-side="right"
        onMouseDown={keep}
        onClick={onShow}
        style={{ "--done": share === null ? 0.25 : share } as CSSProperties}
      >
        <svg className={going && share === null ? "dock-ring is-counting" : "dock-ring"} viewBox="0 0 40 40" aria-hidden="true">
          <circle className="dock-ring-track" cx="20" cy="20" r="18" pathLength={100} />
          <circle className="dock-ring-done" cx="20" cy="20" r="18" pathLength={100} />
        </svg>
        <Picture src={picture} className="dock-rail-thumb" />
        {!going && <span className="dock-rail-mark">{summary?.tone === "ok" ? <OkBadge size={14} /> : <BadgeAlertIcon size={14} />}</span>}
      </button>
    );
  }

  const show = (under: ReactNode) => (
    <button type="button" className="dock-show" aria-label={t("folder.dock.showLabel", { folder: run.name })} data-tip={t("folder.dock.showTip")} onMouseDown={keep} onClick={onShow}>
      <span className="dock-thumb">
        <Picture src={picture} className="dock-thumb-img" />
        {!going && <span className="dock-thumb-mark">{summary?.tone === "ok" ? <OkBadge size={15} /> : <BadgeAlertIcon size={15} />}</span>}
      </span>
      <span className="dock-text">
        <span className="dock-title" data-tip={whole} data-tip-overflow={whole === what ? true : undefined}>
          {what}
        </span>
        {under}
      </span>
    </button>
  );
  const dismiss = (
    <button type="button" className="dock-x" aria-label={t("folder.stage.hideSummaryLabel")} data-tip={t("folder.stage.hideSummary")} onMouseDown={keep} onClick={onDismiss}>
      <XIcon size={12} />
    </button>
  );
  const stop = (
    <button type="button" className="dock-stop" disabled={run.stopping} onMouseDown={keep} onClick={onStop}>
      {run.stopping ? <LoaderIcon size={12} /> : null}
      {run.stopping ? t("folder.stage.stopping") : t("folder.stage.stop")}
    </button>
  );
  const bar = (
    <span className={share === null ? "dock-bar is-counting" : "dock-bar"} role="progressbar" aria-label={what} aria-valuetext={progress} aria-valuemin={0} aria-valuemax={run.counted ? run.total : undefined} aria-valuenow={run.counted ? run.done : undefined}>
      <span style={share === null ? undefined : { width: `${share * 100}%` }} />
    </span>
  );
  const said = summary?.detail ? <span className="dock-said">{whole}</span> : null;
  const folder = (
    <span className="dock-folder" data-tip={run.root} data-tip-overflow>
      <FolderOpenIcon size={12} />
      <span className="dock-folder-name">{run.name}</span>
    </span>
  );

  if (short) {
    // One row, with the bar along its foot: how far it has got under what it's doing, and once
    // it has ended, where under how it went, with what comes next in the folder panel.
    return (
      <section className={`dock is-compact ${tone}`} aria-label={t("folder.dock.label")}>
        {show(going ? <span className="dock-count">{progress}</span> : folder)}
        {going ? stop : dismiss}
        {going && bar}
        {said}
      </section>
    );
  }

  return (
    <section className={`dock ${tone}`} aria-label={t("folder.dock.label")}>
      <div className="dock-head">
        {show(folder)}
        {!going && dismiss}
      </div>
      {going ? (
        <>
          {bar}
          <div className="dock-foot">
            <span className="dock-count">{progress}</span>
            {stop}
          </div>
        </>
      ) : (
        <>
          {said}
          {(canCarryOn(run) || canRetry(run) || canUndo(run)) && (
            <div className="dock-actions">
              {canCarryOn(run) && (
                <button type="button" className="chip-btn" onMouseDown={keep} onClick={onCarryOn}>
                  {t("folder.run.carryOn")}
                </button>
              )}
              {canRetry(run) && (
                <button type="button" className="chip-btn" onMouseDown={keep} onClick={onRetry}>
                  <RotateCcwIcon size={12} />
                  {t("folder.stage.tryAgain", { count: run.failed })}
                </button>
              )}
              {canUndo(run) && (
                <button type="button" className="chip-btn" onMouseDown={keep} onClick={onUndo}>
                  {t("folder.dock.undo")}
                </button>
              )}
            </div>
          )}
        </>
      )}
    </section>
  );
}

/** What a run that's going is doing, in a few words: "Applying Dune", or "Stopping". */
function doing(t: ReturnType<typeof useT>, run: TreeRun, skinName: string | null): string {
  return run.stopping ? t("folder.stage.stopping") : runDoing(run, skinName);
}

function Picture({ src, className }: { src: string | null; className: string }) {
  return src ? (
    <img className={className} src={src} alt="" draggable={false} />
  ) : (
    <span className={className}>
      <FolderOpenIcon size={16} />
    </span>
  );
}
