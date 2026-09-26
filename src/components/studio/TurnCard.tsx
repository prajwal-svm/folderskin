import { useEffect, useRef, useState, type CSSProperties } from "react";
import type { Skin } from "../../lib/tauri";
import { clip } from "../../lib/names";
import { worthRetrying } from "../../lib/aiError";
import { progressOf, refRole, type Turn } from "../../state/chats";
import type { ShapeInfo } from "../../lib/shapes";
import { FolderGhost, IconGhost } from "../FolderGhost";
import { OkBadge } from "../OkBadge";
import { ChevronDownIcon, StopIcon, TerminalIcon } from "../icons/composer";
import { CopyIcon } from "../icons/copy";
import { useT } from "../../i18n";
import { formatNumber } from "../../i18n/format";
import { explain } from "../../lib/sentences";
import { madeWith, providerName } from "../../lib/providerNames";

/** What the chat can do for a turn, from the card's buttons. */
export type TurnActions = {
  /** Asks the same again, with the same provider, model and pictures. */
  again: (turn: Turn) => void;
  /** Puts the turn's words back in the box, to change them. */
  reword: (turn: Turn) => void;
  stop: (turn: Turn) => void;
  /** Opens the provider settings, at `provider`. */
  settings: (provider: string) => void;
  /** Shows the picture on the folder, on the right. */
  preview: (skin: Skin) => void;
  apply: (skin: Skin) => void;
  chooseFolder: () => void;
  menu: (skin: Skin, anchor: HTMLElement) => void;
  /** Copies the question for Claude about a failure. */
  copy: (text: string) => void;
};

const secondsSince = (from: number, to = Date.now()) => Math.max(0, Math.floor((to - from) / 1000));
/** "42s", "3m 05s": how long a request has taken, in the language's short units. */
function useDuration() {
  const t = useT();
  return (s: number) =>
    s < 60 ? t("ai.turn.seconds", { seconds: formatNumber(s) }) : t("ai.turn.minutes", { minutes: formatNumber(Math.floor(s / 60)), seconds: String(s % 60).padStart(2, "0") });
}

/** The log a request wrote, behind a disclosure: there to look at, never in the way. */
function Log({ lines, open, onToggle }: { lines: string[]; open: boolean; onToggle: () => void }) {
  const t = useT();
  const box = useRef<HTMLPreElement>(null);
  useEffect(() => {
    if (open && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [open, lines.length]);
  return (
    <div className="turn-log">
      <button type="button" className="turn-log-toggle" aria-expanded={open} onClick={onToggle}>
        <TerminalIcon size={13} />
        {open ? t("ai.turn.hideDetails") : t("ai.turn.details")}
        <span className="turn-log-chevron" aria-hidden="true">
          <ChevronDownIcon size={13} />
        </span>
      </button>
      {open && (
        <pre className="turn-log-lines" ref={box} tabIndex={0} aria-label={t("ai.turn.logLabel")}>
          {lines.join("\n")}
        </pre>
      )}
    </div>
  );
}

/**
 * The picture developing: the shape it's made for, blank, with light running through it. A
 * folder is drawn as its system draws it and the light kept inside its outline; a free icon,
 * which has no folder, is a soft square.
 */
function Develop({ shape }: { shape: ShapeInfo | undefined }) {
  const free = shape?.family === "free";
  const look = shape?.system === "windows" ? "windows" : shape?.system === "mac" ? "mac" : undefined;
  const mask = !free && shape?.thumbnail ? `url("${shape.thumbnail}")` : undefined;
  return (
    <div className={free ? "develop is-free" : "develop"}>
      {free ? <IconGhost className="develop-ghost" /> : <FolderGhost className="develop-ghost" look={look} />}
      <span className="develop-light" aria-hidden="true" style={mask ? ({ maskImage: mask, WebkitMaskImage: mask } as CSSProperties) : undefined} />
    </div>
  );
}

/** A picture on its way: the shape developing, what's happening now, how far it is, and Stop. */
function Working({ turn, shape, onStop }: { turn: Turn; shape: ShapeInfo | undefined; onStop: () => void }) {
  const t = useT();
  const duration = useDuration();
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 500);
    return () => window.clearInterval(timer);
  }, []);
  const [logOpen, setLogOpen] = useState(false);
  const progress = progressOf(turn);
  const stopping = turn.stage === "Stopping";
  const detail = turn.download ? `${turn.download.file}` : turn.step ? t("ai.turn.step", { done: turn.step.done, total: turn.step.total }) : null;
  return (
    <div className="turn-result is-developing" aria-live="polite">
      <Develop shape={shape} />
      <div className="turn-meta">
        <p className="turn-name develop-step" key={turn.stage ?? "start"}>
          {stopping ? t("ai.turn.stopping") : turn.stage ? explain(turn.stage) : t("ai.turn.sending", { provider: providerName(turn.where.split(" · ")[0]) })}
        </p>
        <div className={progress === null ? "turn-progress is-waiting" : "turn-progress"} style={{ "--done": `${Math.round((progress ?? 0) * 100)}%` } as CSSProperties} aria-hidden="true">
          <span />
        </div>
        <p className="turn-where">
          {[detail, madeWith(turn.where), duration(secondsSince(turn.started, now))].filter(Boolean).join(" · ")}
        </p>
        <div className="turn-actions">
          <button type="button" className="btn btn-secondary btn-sm" disabled={stopping} onClick={onStop}>
            <StopIcon size={12} />
            {stopping ? t("ai.turn.stopping") : t("folder.stage.stop")}
          </button>
        </div>
        {turn.log && turn.log.length > 0 && <Log lines={turn.log} open={logOpen} onToggle={() => setLogOpen((o) => !o)} />}
      </div>
    </div>
  );
}

/** What the chat offers when a request fails, by what went wrong. */
function Failed({ turn, act }: { turn: Turn; act: TurnActions }) {
  const t = useT();
  const [logOpen, setLogOpen] = useState(false);
  const error = turn.error ?? { code: "failed", message: t("ai.turn.didntFinish") };
  const provider = providerName(turn.where.split(" · ")[0]);
  const primary =
    error.code === "missing_key"
      ? { label: t("ai.turn.addKey", { provider }), run: () => act.settings(turn.provider) }
      : error.code === "unauthorized"
        ? { label: t("ai.turn.checkKey"), run: () => act.settings(turn.provider) }
        : error.code === "local_not_ready" || error.code === "runtime_failed_to_start"
          ? { label: t("ai.turn.setUpLocal"), run: () => act.settings("local") }
          : error.code === "refused"
            ? { label: t("ai.turn.reword"), run: () => act.reword(turn) }
            : null;
  return (
    <div className="turn-error" role="alert">
      <p className="turn-error-text">{explain(error.message)}</p>
      {error.fix && error.fix.length > 0 && (
        <ul className="turn-fix">
          {error.fix.map((f) => (
            <li key={f}>{explain(f)}</li>
          ))}
        </ul>
      )}
      <div className="turn-actions">
        {primary && (
          <button type="button" className="btn btn-primary btn-sm" onClick={primary.run}>
            {primary.label}
          </button>
        )}
        {worthRetrying(error.code) && (
          <button type="button" className={primary ? "btn btn-secondary btn-sm" : "btn btn-primary btn-sm"} onClick={() => act.again(turn)}>
            {t("community.tryAgain")}
          </button>
        )}
        {error.ask && (
          <button
            type="button"
            className="btn btn-ghost btn-sm"
            data-tip={t("ai.turn.askTip")}
            onClick={() => act.copy(error.ask!)}
          >
            <CopyIcon size={13} />
            {t("ai.turn.ask")}
          </button>
        )}
      </div>
      {turn.log && turn.log.length > 0 && <Log lines={turn.log} open={logOpen} onToggle={() => setLogOpen((o) => !o)} />}
    </div>
  );
}

/**
 * One request in the chat: what was asked (with its reference pictures), and then the folder
 * developing, the result with what to do with it, what went wrong, or that it was stopped.
 */
export function TurnCard({
  turn,
  live,
  shape,
  shapeLabel,
  folderName,
  onFolder,
  applied,
  act,
}: {
  turn: Turn;
  /** The picture as the library has it now; undefined once it has been deleted there. */
  live: Skin | undefined;
  /** The shape it's made for, as the chat has it. */
  shape: ShapeInfo | undefined;
  /** What it was made for and in, in words: "Windows folder · Woodblock print"; null for a card from before shapes. */
  shapeLabel: string | null;
  /** The folder the chat's pictures are for. */
  folderName: string | null;
  /** This picture is the one on show on the folder, on the right. */
  onFolder: boolean;
  /** This picture was applied to the folder. */
  applied: boolean;
  act: TurnActions;
}) {
  const t = useT();
  const duration = useDuration();
  const [logOpen, setLogOpen] = useState(false);
  return (
    <article className="turn" data-status={turn.status}>
      <div className="turn-ask">
        <p>{turn.idea}</p>
        {shapeLabel && <p className="turn-for">{shapeLabel}</p>}
        {turn.refs.length > 0 && (
          <div className="turn-refs">
            {turn.refs.map((r) => (
              <img key={r.id} src={r.thumb} alt="" data-tip={refRole(r) === "subject" ? r.name : `${r.name} · ${t(`ai.refRole.${refRole(r)}`)}`} draggable={false} />
            ))}
          </div>
        )}
      </div>
      {turn.status === "working" && <Working turn={turn} shape={shape} onStop={() => act.stop(turn)} />}
      {turn.status === "error" && <Failed turn={turn} act={act} />}
      {turn.status === "stopped" && (
        <div className="turn-stopped">
          <p>{turn.finished ? t("ai.turn.stoppedAfter", { time: duration(secondsSince(turn.started, turn.finished)) }) : t("ai.turn.stopped")}</p>
          <button type="button" className="btn btn-secondary btn-sm" onClick={() => act.again(turn)}>
            {t("community.tryAgain")}
          </button>
        </div>
      )}
      {turn.status === "done" &&
        (live ? (
          <div className="turn-result">
            <img className="turn-img" src={live.thumbnail} alt="" draggable={false} />
            <div className="turn-meta">
              <div className="turn-title">
                <p className="turn-name" data-tip={live.name} data-tip-overflow>
                  {live.name}
                </p>
                <button
                  type="button"
                  className="icon-btn turn-more"
                  aria-label={t("library.tile.optionsLabel", { name: live.name })}
                  aria-haspopup="dialog"
                  data-tip={t("ai.turn.options")}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={(e) => act.menu(live, e.currentTarget)}
                >
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                    <circle cx="5" cy="12" r="2" />
                    <circle cx="12" cy="12" r="2" />
                    <circle cx="19" cy="12" r="2" />
                  </svg>
                </button>
              </div>
              {live.tags.length > 0 && (
                <div className="turn-tags">
                  {live.tags.map((tag) => (
                    <span className="tag-chip" key={tag}>
                      {tag}
                    </span>
                  ))}
                </div>
              )}
              <p className="turn-where">
                {madeWith(turn.where)}
                {turn.finished ? ` · ${duration(secondsSince(turn.started, turn.finished))}` : ""} · {t("ai.turn.inYours")}
              </p>
              <div className="turn-actions">
                {folderName ? (
                  applied ? (
                    <span className="turn-applied">
                      <OkBadge size={16} /> {t("ai.turn.on", { name: clip(folderName, 24) })}
                    </span>
                  ) : (
                    <button type="button" className="btn btn-primary btn-sm" onMouseDown={(e) => e.preventDefault()} onClick={() => act.apply(live)}>
                      {t("ai.turn.applyTo", { name: clip(folderName, 24) })}
                    </button>
                  )
                ) : (
                  <button type="button" className="btn btn-primary btn-sm" onMouseDown={(e) => e.preventDefault()} onClick={act.chooseFolder}>
                    {t("common.dialog.chooseFolder")}
                  </button>
                )}
                {folderName && !applied && (
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    disabled={onFolder}
                    data-tip={onFolder ? undefined : t("ai.turn.previewTip", { name: clip(folderName, 24) })}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => act.preview(live)}
                  >
                    {onFolder ? t("ai.turn.onShow") : t("ai.turn.preview")}
                  </button>
                )}
                <button type="button" className="btn btn-ghost btn-sm" onMouseDown={(e) => e.preventDefault()} onClick={() => act.again(turn)}>
                  {t("ai.turn.another")}
                </button>
              </div>
              {turn.log && turn.log.length > 0 && <Log lines={turn.log} open={logOpen} onToggle={() => setLogOpen((o) => !o)} />}
            </div>
          </div>
        ) : (
          <div className="turn-stopped">
            <p>{t("ai.turn.deleted")}</p>
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => act.again(turn)}>
              {t("ai.turn.makeAgain")}
            </button>
          </div>
        ))}
    </article>
  );
}
