import { useCallback, useEffect, useId, useRef, useState, type CSSProperties } from "react";
import { api, LOCAL_SETUP_JOB, type LocalStatus } from "../../lib/tauri";
import { aiFailure, worthRetrying } from "../../lib/aiError";
import { formatBytes } from "../../i18n/format";
import { useT } from "../../i18n";
import { explain } from "../../lib/sentences";
import { Rich } from "../../i18n/Rich";
import { duration, spaceShort, whatItTakes } from "../../lib/localSetup";
import type { AiEvent, TurnError } from "../../state/chats";
import { heard, setupBegan, useSetupProgress, wholeDone } from "../../state/localSetupRun";
import { Confirm } from "../Confirm";
import { CpuIcon } from "../icons/composer";
import { CopyIcon } from "../icons/copy";
import { DeleteIcon } from "../icons/delete";
import { InfoIcon } from "../icons/info";
import { LoaderIcon } from "../icons/loader";

/**
 * The Local Model, with no key and no account: the model, and folded away beneath it the machine
 * it runs on, what it runs with, how long the last picture took there and where it is stored (the
 * path veiled until it's pointed at, since it names the user), with the model's removal at the
 * bottom. One button downloads and checks everything it needs, showing each file as it comes,
 * once the disk has room for it. It can be stopped part-way; what was downloaded is kept, and
 * setting up again carries on from there. A setup that was already under way when the panel
 * opened (it was closed, or another provider picked) is joined, so its progress and its Stop are
 * back. Whether it's ready shows on its tile in the provider list, not here.
 */
export function LocalSetup({ onChanged, copy }: { onChanged: () => void; copy: (text: string) => void }) {
  const t = useT();
  const [status, setStatus] = useState<LocalStatus | null>(null);
  const [problem, setProblem] = useState<TurnError | null>(null);
  // Kept for the window (state/localSetupRun.ts), so opened again part-way it counts on.
  const setup = useSetupProgress();
  const [stopping, setStopping] = useState(false);
  const [factsOpen, setFactsOpen] = useState(false);
  /** Asking whether to remove the model, or removing it. */
  const [removal, setRemoval] = useState<"ask" | "removing" | null>(null);
  /** Likewise for the model files an earlier setup left that the model doesn't use now. */
  const [leftovers, setLeftovers] = useState<"ask" | "removing" | null>(null);
  /** A setup that ended short is being asked what's left of the download, which the next one counts. */
  const [recounting, setRecounting] = useState(false);
  const looked = useRef<(status: LocalStatus) => void>(() => {});
  const factsId = useId();

  useEffect(() => {
    let live = true;
    api
      .aiLocalStatus()
      .then((s) => {
        if (!live) return;
        setStatus(s);
        looked.current(s);
      })
      .catch((e) => live && setProblem(aiFailure(e)));
    return () => {
      live = false;
    };
  }, []);

  const start = useCallback(async (from?: LocalStatus) => {
    setProblem(null);
    setStopping(false);
    const ended = setupBegan((from ?? status)?.download_bytes ?? 0);
    const onEvent = (e: AiEvent) => heard(e);
    try {
      setStatus(await api.aiLocalSetup(onEvent));
      onChanged();
    } catch (e) {
      setProblem(aiFailure(e));
      // What a stopped or failed setup downloaded is kept: the line beside the button should say
      // what is left, not what there was before it started, and setting up again counts what is
      // left, so it waits for that.
      setRecounting(true);
      api
        .aiLocalStatus()
        .then(setStatus)
        .catch(() => {})
        .finally(() => setRecounting(false));
    } finally {
      ended();
      setStopping(false);
    }
  }, [onChanged, status]);
  useEffect(() => {
    looked.current = (s) => {
      // Asking to set up while a setup runs joins it (ai_local_setup), with its progress from here on.
      if (s.setting_up) void start(s);
      // The status has just asked the runtime whether it starts; the provider list goes by that too.
      else onChanged();
    };
  }, [start, onChanged]);

  const stop = useCallback(() => {
    setStopping(true);
    api.aiCancel(LOCAL_SETUP_JOB).catch(() => setStopping(false));
  }, []);

  const remove = useCallback(async () => {
    setProblem(null);
    setRemoval("removing");
    try {
      // Done is shown by what follows: the panel offers to set it up again.
      setStatus(await api.aiLocalRemove());
      onChanged();
    } catch (e) {
      setProblem(aiFailure(e));
    } finally {
      setRemoval(null);
    }
  }, [onChanged]);

  const removeLeftovers = useCallback(async () => {
    setProblem(null);
    setLeftovers("removing");
    try {
      setStatus(await api.aiLocalRemoveUnused());
    } catch (e) {
      setProblem(aiFailure(e));
    } finally {
      setLeftovers(null);
    }
  }, []);

  if (!status && !problem) {
    return (
      <p className="local-note">
        <LoaderIcon size={14} /> {t("ai.local.looking")}
      </p>
    );
  }

  // The bar is the whole download's when it's known how much that is, the file's otherwise.
  const share = !setup ? 0 : setup.whole > 0 ? Math.min(100, (wholeDone(setup) / setup.whole) * 100) : setup.total > 0 ? Math.min(100, (setup.done / setup.total) * 100) : 0;
  const short = status ? spaceShort(status) : null;
  // A stop that was asked for isn't a failure: it says what was kept, with one way on.
  const stopped = problem?.code === "stopped";
  return (
    <div className="local-setup">
      {status && (
        <div className={factsOpen ? "local-facts is-open" : "local-facts"}>
          <button type="button" className="local-machine" aria-expanded={factsOpen} aria-controls={factsId} onClick={() => setFactsOpen((open) => !open)}>
            <span className="local-model">
              <CpuIcon size={15} />
              <span className="local-model-name">{status.model}</span>
              <span className="local-muted">
                {explain(status.quality)} · {formatBytes(status.model_bytes)}
              </span>
            </span>
            <span className="local-info" aria-hidden="true">
              <InfoIcon size={15} />
            </span>
          </button>
          <div className="local-more" id={factsId} role="region" aria-label={t("ai.local.aboutLabel")} inert={!factsOpen}>
            <div className="local-more-inner">
              <dl className="local-list">
                <div>
                  <dt>{t("ai.local.machine")}</dt>
                  <dd>{explain(status.device)}</dd>
                </div>
                <div>
                  <dt>{t("ai.local.runsWith")}</dt>
                  <dd>{status.backend}</dd>
                </div>
                <div>
                  <dt>{t("ai.local.lastPicture")}</dt>
                  <dd>
                    {status.seconds_per_image === null ? (
                      <span className="local-muted">{t("ai.local.afterFirst")}</span>
                    ) : (
                      <Rich k="ai.local.took" vars={{ time: duration(status.seconds_per_image) }} tags={{ muted: (s) => <span className="local-muted">{s}</span> }} />
                    )}
                  </dd>
                </div>
                <div>
                  <dt>{t("ai.local.storedAt")}</dt>
                  <dd>
                    {/* It names the user: veiled until it's pointed at or reached with the keyboard. */}
                    <span className="local-path" tabIndex={factsOpen ? 0 : -1}>
                      {status.home}
                    </span>
                  </dd>
                </div>
              </dl>
              {status.kept_bytes > 0 && !setup && (
                <div className="local-more-foot">
                  <button type="button" className="btn btn-ghost btn-sm local-remove-btn" disabled={removal === "removing"} onClick={() => setRemoval("ask")}>
                    {removal === "removing" ? <LoaderIcon size={13} /> : <DeleteIcon size={13} />}
                    {removal === "removing" ? t("ai.local.removingModel") : t("ai.local.removeModel")}
                  </button>
                </div>
              )}
            </div>
          </div>
          {status.note && <p className="local-note">{explain(status.note)}</p>}
        </div>
      )}
      {status && status.unused_bytes > 0 && !setup && (
        // An earlier build's files (the other tier's, Z-Image Turbo's) that setting up again
        // doesn't use or take away: said here, not only in the fold, with the way to get the room back.
        <div className="local-go">
          <span className="local-note">{t("ai.local.leftovers", { size: formatBytes(status.unused_bytes) })}</span>
          <button type="button" className="btn btn-secondary btn-sm" disabled={leftovers === "removing"} onClick={() => setLeftovers("ask")}>
            {leftovers === "removing" ? <LoaderIcon size={13} /> : <DeleteIcon size={13} />}
            {leftovers === "removing" ? t("ai.local.removingThem") : t("ai.local.removeThem")}
          </button>
        </div>
      )}
      {status && !status.ready && status.can_set_up && !setup && !stopped && (
        <div className="local-go">
          <span className={short ? "local-note is-warn" : "local-note"}>{short ?? whatItTakes(status)}</span>
          <button type="button" className="btn btn-primary" disabled={short !== null || recounting} onClick={() => void start()}>
            {t("ai.turn.setUpLocal")}
          </button>
        </div>
      )}
      {setup && (
        <div className="local-progress" role="status" aria-live="polite">
          <p className="local-stage">
            <LoaderIcon size={14} /> {stopping ? t("ai.turn.stopping") : setup.stage ? explain(setup.stage) : t("ai.local.gettingReady")}
            <button type="button" className="btn btn-ghost btn-sm local-stop" disabled={stopping} onClick={stop}>
              {t("folder.stage.stop")}
            </button>
          </p>
          <div className="turn-progress" style={{ "--done": `${share}%` } as CSSProperties} aria-hidden="true">
            <span />
          </div>
          <p className="local-note">
            {[
              // The file the stage above names, once it's coming; between files, the whole alone.
              setup.file
                ? t("ai.local.fileProgress", { file: setup.file, done: formatBytes(setup.done), total: formatBytes(setup.total) })
                : wholeDone(setup) === 0
                  ? t("ai.local.starting")
                  : null,
              setup.whole > 0 ? t("ai.local.wholeProgress", { done: formatBytes(Math.min(wholeDone(setup), setup.whole)), total: formatBytes(setup.whole) }) : null,
            ]
              .filter(Boolean)
              .join(" · ")}
          </p>
        </div>
      )}
      {stopped && !setup && (
        <div className="local-go" role="status">
          <span className="local-note">{explain(problem.message)}</span>
          {status?.can_set_up && (
            <button type="button" className="btn btn-primary" disabled={recounting} onClick={() => void start()}>
              {recounting && <LoaderIcon size={15} />}
              {t("ai.local.carryOn")}
            </button>
          )}
        </div>
      )}
      {problem && !stopped && (
        <div className="turn-error" role="alert">
          <p className="turn-error-text">{explain(problem.message)}</p>
          {problem.fix && (
            <ul className="turn-fix">
              {problem.fix.map((f) => (
                <li key={f}>{explain(f)}</li>
              ))}
            </ul>
          )}
          <div className="turn-actions">
            {status?.can_set_up && worthRetrying(problem.code) && (
              <button type="button" className="btn btn-secondary btn-sm" disabled={recounting} onClick={() => void start()}>
                {t("community.tryAgain")}
              </button>
            )}
            {problem.ask && (
              <button type="button" className="btn btn-ghost btn-sm" onClick={() => copy(problem.ask!)}>
                <CopyIcon size={13} />
                {t("ai.turn.ask")}
              </button>
            )}
          </div>
        </div>
      )}
      {leftovers === "ask" && status && (
        <Confirm
          title={t("ai.local.removeLeftoversTitle")}
          text={t("ai.local.removeLeftoversText", { size: formatBytes(status.unused_bytes) })}
          action={t("community.remove.action")}
          onCancel={() => setLeftovers(null)}
          onConfirm={() => void removeLeftovers()}
        />
      )}
      {removal === "ask" && status && (
        <Confirm
          title={t("ai.local.removeTitle")}
          text={t("ai.local.removeText", { size: formatBytes(status.kept_bytes) })}
          action={t("community.remove.action")}
          onCancel={() => setRemoval(null)}
          onConfirm={() => void remove()}
        />
      )}
    </div>
  );
}
