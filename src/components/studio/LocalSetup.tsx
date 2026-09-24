import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { api, LOCAL_SETUP_JOB, type LocalStatus } from "../../lib/tauri";
import { aiFailure, worthRetrying } from "../../lib/aiError";
import { formatBytes } from "../../lib/tree";
import { machineDetails, whatItTakes } from "../../lib/localSetup";
import type { AiEvent, TurnError } from "../../state/chats";
import { Confirm } from "../Confirm";
import { OkBadge } from "../OkBadge";
import { CpuIcon, TerminalIcon } from "../icons/composer";
import { CopyIcon } from "../icons/copy";
import { DeleteIcon } from "../icons/delete";
import { InfoIcon } from "../icons/info";
import { LoaderIcon } from "../icons/loader";

type Setup = { stage: string; file: string | null; done: number; total: number; log: string[] };

/**
 * The Local Model, with no key and no account: the machine it runs on (and, behind the info
 * button, what it runs with, how long the last picture took here and where it is kept), and one
 * button that downloads and checks everything it needs, showing each file as it comes and a log
 * for anyone who wants to see what it's doing. It can be stopped part-way; what was downloaded
 * is kept, and setting up again carries on from there. A setup that was already under way when
 * the panel opened (it was closed, or another provider picked) is joined, so its progress and its
 * Stop are back. Once anything is downloaded, the model can be removed again to free the space.
 */
export function LocalSetup({ onChanged, copy }: { onChanged: () => void; copy: (text: string, what: string) => void }) {
  const [status, setStatus] = useState<LocalStatus | null>(null);
  const [problem, setProblem] = useState<TurnError | null>(null);
  const [setup, setSetup] = useState<Setup | null>(null);
  const [logOpen, setLogOpen] = useState(false);
  const [stopping, setStopping] = useState(false);
  /** Asking whether to remove the model, removing it, or what removing it freed. */
  const [removal, setRemoval] = useState<{ step: "ask" | "removing" } | { step: "done"; freed: number } | null>(null);
  const looked = useRef<(status: LocalStatus) => void>(() => {});

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

  const start = useCallback(async () => {
    setProblem(null);
    setStopping(false);
    setSetup({ stage: "Getting ready", file: null, done: 0, total: 0, log: [] });
    const onEvent = (e: AiEvent) =>
      setSetup((s) => {
        if (!s) return s;
        switch (e.type) {
          case "stage":
            return { ...s, stage: e.message };
          case "download":
            return { ...s, file: e.file, done: e.done, total: e.total };
          case "progress":
            return { ...s, done: e.step, total: e.steps };
          case "log":
            return { ...s, log: [...s.log.slice(-300), e.level === "info" ? e.message : `${e.level}: ${e.message}`] };
        }
      });
    try {
      setStatus(await api.aiLocalSetup(onEvent));
      onChanged();
    } catch (e) {
      setProblem(aiFailure(e));
      // What a stopped or failed setup downloaded is kept: the line under the button should say
      // what is left, not what there was before it started.
      api
        .aiLocalStatus()
        .then(setStatus)
        .catch(() => {});
    } finally {
      setSetup(null);
      setStopping(false);
    }
  }, [onChanged]);
  useEffect(() => {
    looked.current = (s) => {
      // Asking to set up while a setup runs joins it (ai_local_setup), with its progress from here on.
      if (s.setting_up) void start();
      // The status has just asked the runtime whether it starts; the provider list goes by that too.
      else onChanged();
    };
  }, [start, onChanged]);

  const stop = useCallback(() => {
    setStopping(true);
    api.aiCancel(LOCAL_SETUP_JOB).catch(() => setStopping(false));
  }, []);

  const remove = useCallback(async () => {
    const freed = status?.kept_bytes ?? 0;
    setProblem(null);
    setRemoval({ step: "removing" });
    try {
      setStatus(await api.aiLocalRemove());
      setRemoval({ step: "done", freed });
      onChanged();
    } catch (e) {
      setRemoval(null);
      setProblem(aiFailure(e));
    }
  }, [status, onChanged]);

  if (!status && !problem) {
    return (
      <p className="local-note">
        <LoaderIcon size={14} /> Looking at your machine
      </p>
    );
  }

  const share = setup && setup.total > 0 ? Math.min(100, (setup.done / setup.total) * 100) : 0;
  return (
    <div className="local-setup">
      {status && (
        <div className="local-facts">
          <div className="local-machine">
            <span className="local-chip">
              <CpuIcon size={15} />
              {status.device}
            </span>
            <button
              type="button"
              className="icon-btn local-info"
              aria-label={`about your machine: ${machineDetails(status).replaceAll("\n", ", ")}`}
              data-tip={machineDetails(status)}
              data-tip-side="left"
            >
              <InfoIcon size={15} />
            </button>
          </div>
          {status.note && <p className="local-note">{status.note}</p>}
        </div>
      )}
      {status?.ready && !setup && (
        <p className="local-ready">
          <OkBadge size={17} playOnMount /> Set up and ready.
        </p>
      )}
      {status && !status.ready && status.can_set_up && !setup && (
        <div className="local-go">
          <button type="button" className="btn btn-primary" onClick={() => void start()}>
            Set up the local model
          </button>
          <span className="local-note">{whatItTakes(status)}</span>
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
          </p>
          {setup.log.length > 0 && (
            <div className="turn-log">
              <button type="button" className="turn-log-toggle" aria-expanded={logOpen} onClick={() => setLogOpen((o) => !o)}>
                <TerminalIcon size={13} />
                {logOpen ? "Hide the details" : "Details"}
              </button>
              {logOpen && <pre className="turn-log-lines">{setup.log.join("\n")}</pre>}
            </div>
          )}
        </div>
      )}
      {problem && (
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
            {status?.can_set_up && (problem.code === "stopped" || worthRetrying(problem.code)) && (
              <button type="button" className="btn btn-secondary btn-sm" onClick={() => void start()}>
                {problem.code === "stopped" ? "Carry on setting up" : "Try again"}
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
      {status && !setup && (status.kept_bytes > 0 || removal !== null) && (
        <div className="local-remove">
          {removal?.step === "done" ? (
            <p className="local-note">Removed. {removal.freed > 0 ? `${formatBytes(removal.freed)} is free again.` : "Nothing was left to remove."}</p>
          ) : (
            <>
              <button type="button" className="btn btn-ghost btn-sm local-remove-btn" disabled={removal?.step === "removing"} onClick={() => setRemoval({ step: "ask" })}>
                {removal?.step === "removing" ? <LoaderIcon size={13} /> : <DeleteIcon size={13} />}
                {removal?.step === "removing" ? "Removing the model" : "Remove the model"}
              </button>
              <span className="local-note">Frees {formatBytes(status.kept_bytes)} on your machine.</span>
            </>
          )}
        </div>
      )}
      {removal?.step === "ask" && status && (
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
