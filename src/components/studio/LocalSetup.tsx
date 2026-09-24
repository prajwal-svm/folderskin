import { useCallback, useEffect, useId, useRef, useState, type CSSProperties } from "react";
import { api, LOCAL_SETUP_JOB, type LocalStatus } from "../../lib/tauri";
import { aiFailure, worthRetrying } from "../../lib/aiError";
import { formatBytes } from "../../lib/tree";
import { duration, spaceShort, whatItTakes } from "../../lib/localSetup";
import type { AiEvent, TurnError } from "../../state/chats";
import { setupBegan } from "../../state/localSetupRun";
import { Confirm } from "../Confirm";
import { CpuIcon } from "../icons/composer";
import { CopyIcon } from "../icons/copy";
import { DeleteIcon } from "../icons/delete";
import { InfoIcon } from "../icons/info";
import { LoaderIcon } from "../icons/loader";

/**
 * A setup under way: the file coming in, and for the whole download how much there was to fetch
 * when it started and how much of each file has come since (from where that file resumed).
 */
type Setup = { stage: string; file: string | null; done: number; total: number; whole: number; from: Record<string, number>; got: Record<string, number> };

/** How much of the whole download has come so far. */
const wholeDone = (setup: Setup) => Object.values(setup.got).reduce((sum, n) => sum + n, 0);

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
export function LocalSetup({ onChanged, copy }: { onChanged: () => void; copy: (text: string, what: string) => void }) {
  const [status, setStatus] = useState<LocalStatus | null>(null);
  const [problem, setProblem] = useState<TurnError | null>(null);
  const [setup, setSetup] = useState<Setup | null>(null);
  const [stopping, setStopping] = useState(false);
  const [factsOpen, setFactsOpen] = useState(false);
  /** Asking whether to remove the model, or removing it. */
  const [removal, setRemoval] = useState<"ask" | "removing" | null>(null);
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
    const whole = (from ?? status)?.download_bytes ?? 0;
    setSetup({ stage: "Getting ready", file: null, done: 0, total: 0, whole, from: {}, got: {} });
    const ended = setupBegan();
    const onEvent = (e: AiEvent) =>
      setSetup((s) => {
        if (!s) return s;
        switch (e.type) {
          case "stage":
            return { ...s, stage: e.message };
          case "download": {
            // A file that resumes starts where it left off: only what comes now counts.
            const from = e.file in s.from ? s.from : { ...s.from, [e.file]: e.done };
            const got = { ...s.got, [e.file]: Math.max(0, e.done - from[e.file]) };
            return { ...s, file: e.file, done: e.done, total: e.total, from, got };
          }
          case "progress":
            return { ...s, done: e.step, total: e.steps };
          case "log":
            return s;
        }
      });
    try {
      setStatus(await api.aiLocalSetup(onEvent));
      onChanged();
    } catch (e) {
      setProblem(aiFailure(e));
      // What a stopped or failed setup downloaded is kept: the line beside the button should say
      // what is left, not what there was before it started.
      api
        .aiLocalStatus()
        .then(setStatus)
        .catch(() => {});
    } finally {
      ended();
      setSetup(null);
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

  if (!status && !problem) {
    return (
      <p className="local-note">
        <LoaderIcon size={14} /> Looking at your machine
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
                {status.quality} · {formatBytes(status.model_bytes)}
              </span>
            </span>
            <span className="local-info" aria-hidden="true">
              <InfoIcon size={15} />
            </span>
          </button>
          <div className="local-more" id={factsId} role="region" aria-label="about the local model" inert={!factsOpen}>
            <div className="local-more-inner">
              <dl className="local-list">
                <div>
                  <dt>Machine</dt>
                  <dd>{status.device}</dd>
                </div>
                <div>
                  <dt>Runs with</dt>
                  <dd>{status.backend}</dd>
                </div>
                <div>
                  <dt>Last picture</dt>
                  <dd>
                    {status.seconds_per_image === null ? (
                      <span className="local-muted">Shows after your first one</span>
                    ) : (
                      <>
                        {duration(status.seconds_per_image)} <span className="local-muted">on this machine</span>
                      </>
                    )}
                  </dd>
                </div>
                <div>
                  <dt>Stored at</dt>
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
                    {removal === "removing" ? "Removing the model" : "Remove the model"}
                  </button>
                </div>
              )}
            </div>
          </div>
          {status.note && <p className="local-note">{status.note}</p>}
        </div>
      )}
      {status && !status.ready && status.can_set_up && !setup && !stopped && (
        <div className="local-go">
          <span className={short ? "local-note is-warn" : "local-note"}>{short ?? whatItTakes(status)}</span>
          <button type="button" className="btn btn-primary" disabled={short !== null} onClick={() => void start()}>
            Set up the local model
          </button>
        </div>
      )}
      {setup && (
        <div className="local-progress" role="status" aria-live="polite">
          <p className="local-stage">
            <LoaderIcon size={14} /> {stopping ? "Stopping" : setup.stage}
            <button type="button" className="btn btn-ghost btn-sm local-stop" disabled={stopping} onClick={stop}>
              Stop
            </button>
          </p>
          <div className="turn-progress" style={{ "--done": `${share}%` } as CSSProperties} aria-hidden="true">
            <span />
          </div>
          <p className="local-note">
            {setup.file ? `${setup.file}: ${formatBytes(setup.done)} of ${formatBytes(setup.total)}` : "Starting"}
            {setup.whole > 0 && ` · ${formatBytes(Math.min(wholeDone(setup), setup.whole))} of ${formatBytes(setup.whole)} in all`}
          </p>
        </div>
      )}
      {stopped && !setup && (
        <div className="local-go" role="status">
          <span className="local-note">{problem.message}</span>
          {status?.can_set_up && (
            <button type="button" className="btn btn-primary" onClick={() => void start()}>
              Carry on setting up
            </button>
          )}
        </div>
      )}
      {problem && !stopped && (
        <div className="turn-error" role="alert">
          <p className="turn-error-text">{problem.message}</p>
          {problem.fix && (
            <ul className="turn-fix">
              {problem.fix.map((f) => (
                <li key={f}>{f}</li>
              ))}
            </ul>
          )}
          <div className="turn-actions">
            {status?.can_set_up && worthRetrying(problem.code) && (
              <button type="button" className="btn btn-secondary btn-sm" onClick={() => void start()}>
                Try again
              </button>
            )}
            {problem.ask && (
              <button type="button" className="btn btn-ghost btn-sm" onClick={() => copy(problem.ask!, "The question for Claude")}>
                <CopyIcon size={13} />
                Ask Claude to fix it
              </button>
            )}
          </div>
        </div>
      )}
      {removal === "ask" && status && (
        <Confirm
          title="Remove the local model?"
          text={`This deletes the model and everything its setup downloaded (${formatBytes(status.kept_bytes)}). Your skins stay, and you can set it up again whenever you like.`}
          action="Remove"
          onCancel={() => setRemoval(null)}
          onConfirm={() => void remove()}
        />
      )}
    </div>
  );
}
