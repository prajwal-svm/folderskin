import { useCallback, useEffect, useState, type CSSProperties } from "react";
import { api, type LocalStatus } from "../../lib/tauri";
import { aiFailure } from "../../lib/aiError";
import { formatBytes } from "../../lib/tree";
import type { AiEvent, TurnError } from "../../state/chats";
import { OkBadge } from "../OkBadge";
import { CpuIcon, TerminalIcon } from "../icons/composer";
import { CopyIcon } from "../icons/copy";
import { LoaderIcon } from "../icons/loader";

type Setup = { stage: string; file: string | null; done: number; total: number; log: string[] };

/**
 * Pictures made on this computer, with no key and no account: what it runs on here and how long a
 * picture takes, and one button that downloads and checks everything it needs, showing each file
 * as it comes and a log for anyone who wants to see what it's doing.
 */
export function LocalSetup({ onChanged, copy }: { onChanged: () => void; copy: (text: string, what: string) => void }) {
  const [status, setStatus] = useState<LocalStatus | null>(null);
  const [problem, setProblem] = useState<TurnError | null>(null);
  const [setup, setSetup] = useState<Setup | null>(null);
  const [logOpen, setLogOpen] = useState(false);

  useEffect(() => {
    let live = true;
    api
      .aiLocalStatus()
      .then((s) => live && setStatus(s))
      .catch((e) => live && setProblem(aiFailure(e)));
    return () => {
      live = false;
    };
  }, []);

  const start = useCallback(async () => {
    setProblem(null);
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
    } finally {
      setSetup(null);
    }
  }, [onChanged]);

  if (!status && !problem) {
    return (
      <p className="local-note">
        <LoaderIcon size={14} /> Looking at this computer…
      </p>
    );
  }

  const share = setup && setup.total > 0 ? Math.min(100, (setup.done / setup.total) * 100) : 0;
  return (
    <div className="local-setup">
      {status && (
        <div className="local-facts">
          <span className="local-chip">
            <CpuIcon size={15} />
            {status.device}
          </span>
          <dl className="local-list">
            <div>
              <dt>Runs with</dt>
              <dd>{status.backend}</dd>
            </div>
            {status.seconds_per_image !== null && (
              <div>
                <dt>A picture takes</dt>
                <dd>about {status.seconds_per_image < 60 ? `${Math.round(status.seconds_per_image)} seconds` : `${Math.round(status.seconds_per_image / 60)} minutes`}</dd>
              </div>
            )}
            <div>
              <dt>Kept in</dt>
              <dd className="local-path" data-tip={status.home} data-tip-overflow>
                {status.home}
              </dd>
            </div>
          </dl>
          {status.note && <p className="local-note">{status.note}</p>}
        </div>
      )}
      {status?.ready && !setup && (
        <p className="local-ready">
          <OkBadge size={17} playOnMount /> Set up and ready. Nothing you make here leaves this computer.
        </p>
      )}
      {status && !status.ready && !setup && (
        <div className="local-go">
          <button type="button" className="btn btn-primary" onClick={() => void start()}>
            Set up this computer
          </button>
          <span className="local-note">Downloads {formatBytes(status.download_bytes)} once, then works offline.</span>
        </div>
      )}
      {setup && (
        <div className="local-progress" role="status" aria-live="polite">
          <p className="local-stage">
            <LoaderIcon size={14} /> {setup.stage}
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
            {status && (
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
    </div>
  );
}
