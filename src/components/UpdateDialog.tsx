import { Fragment, useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { parseNotes, splitInline } from "../lib/notes";
import { REPO_URL } from "../lib/packs";
import { restartApp, type AvailableUpdate } from "../lib/updater";
import type { UpdateStatus } from "../hooks/useUpdates";
import { Modal } from "./Modal";
import { ExternalLinkIcon } from "./icons/external-link";
import { OkBadge } from "./OkBadge";
import { BadgeAlertIcon } from "./icons/badge-alert";
import { CheckIcon } from "./icons/check";
import { DownloadIcon } from "./icons/download";
import { LoaderIcon } from "./icons/loader";
import { RefreshCwIcon } from "./icons/refresh-cw";

type Phase =
  | { kind: "ready" }
  | { kind: "downloading"; fraction: number | null }
  | { kind: "installing" }
  | { kind: "restarting" }
  | { kind: "failed" }
  /** Installed, but FolderSkin couldn't start it by itself. */
  | { kind: "installed" };

/**
 * A newer FolderSkin: what changed, and one button that downloads it, checks its signature,
 * installs it and restarts. Once it starts it can't be closed or left halfway.
 */
export function UpdateDialog({ update, onClose }: { update: AvailableUpdate; onClose: () => void }) {
  const [phase, setPhase] = useState<Phase>({ kind: "ready" });
  const busy = phase.kind === "downloading" || phase.kind === "installing" || phase.kind === "restarting";

  const run = async () => {
    setPhase({ kind: "downloading", fraction: 0 });
    try {
      await update.install((fraction) => setPhase(fraction === 1 ? { kind: "installing" } : { kind: "downloading", fraction }));
    } catch (e) {
      console.warn("folderskin: the update didn't install:", e);
      setPhase({ kind: "failed" });
      return;
    }
    setPhase({ kind: "restarting" });
    try {
      await restartApp();
    } catch (e) {
      console.warn("folderskin: couldn't restart after the update:", e);
      setPhase({ kind: "installed" });
    }
  };

  const footer = busy ? (
    <Progress phase={phase} />
  ) : phase.kind === "installed" ? (
    <button type="button" className="btn btn-primary" onClick={onClose}>
      Close
    </button>
  ) : (
    <>
      <button type="button" className="btn btn-secondary" onClick={onClose}>
        Later
      </button>
      <button type="button" className="btn btn-primary" onClick={() => void run()}>
        {phase.kind === "failed" ? <RefreshCwIcon size={16} /> : <DownloadIcon size={16} />}
        {phase.kind === "failed" ? "Try again" : "Update and restart"}
      </button>
    </>
  );

  return (
    <Modal
      title="Update available"
      sub={`FolderSkin ${update.version} is out. You have ${__APP_VERSION__}.`}
      onClose={onClose}
      closable={!busy}
      footer={footer}
    >
      {phase.kind === "failed" && (
        <p className="update-problem" role="alert">
          <BadgeAlertIcon size={16} />
          <span>
            The update didn't finish, so nothing changed. Try again, or download it from the{" "}
            <button type="button" className="link-btn update-inline-link" onClick={() => void openUrl(`${REPO_URL}/releases/latest`).catch(() => {})}>
              Releases page <ExternalLinkIcon size={12} />
            </button>
            .
          </span>
        </p>
      )}
      {phase.kind === "installed" && (
        <p className="update-problem is-done" role="status">
          <CheckIcon size={16} />
          <span>
            FolderSkin {update.version} is installed. Quit FolderSkin and open it again to use it.
          </span>
        </p>
      )}
      <ReleaseNotes markdown={update.notes} />
    </Modal>
  );
}

function Progress({ phase }: { phase: Phase }) {
  const fraction = phase.kind === "downloading" ? phase.fraction : 1;
  const label =
    phase.kind === "downloading"
      ? fraction === null
        ? "Downloading"
        : `Downloading ${Math.round(fraction * 100)}%`
      : phase.kind === "installing"
        ? "Installing"
        : "Restarting";
  return (
    <div className="update-progress" role="status">
      <span className="update-progress-label">
        <LoaderIcon size={14} />
        {label}
      </span>
      <span
        className={fraction === null ? "update-bar is-unknown" : "update-bar"}
        role="progressbar"
        aria-label="update"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={fraction === null ? undefined : Math.round(fraction * 100)}
      >
        <span style={fraction === null ? undefined : { transform: `scaleX(${fraction})` }} />
      </span>
    </div>
  );
}

/** The version's notes from CHANGELOG.md, drawn from the little Markdown they use. */
function ReleaseNotes({ markdown }: { markdown: string }) {
  const blocks = useMemo(() => parseNotes(markdown), [markdown]);
  if (blocks.length === 0) return <p className="field-note">No notes came with this version.</p>;
  return (
    <div className="update-notes">
      {blocks.map((b, i) =>
        b.kind === "heading" ? (
          <h3 key={i} className="update-notes-heading">
            {b.text}
          </h3>
        ) : b.kind === "list" ? (
          <ul key={i}>
            {b.items.map((item, j) => (
              <li key={j}>
                <Inline text={item} />
              </li>
            ))}
          </ul>
        ) : (
          <p key={i}>
            <Inline text={b.text} />
          </p>
        ),
      )}
    </div>
  );
}

function Inline({ text }: { text: string }) {
  return (
    <>
      {splitInline(text).map((part, i) =>
        part.kind === "strong" ? <strong key={i}>{part.text}</strong> : part.kind === "code" ? <code key={i}>{part.text}</code> : <Fragment key={i}>{part.text}</Fragment>,
      )}
    </>
  );
}

/**
 * Checks for an update where you ask for one, and answers in place: checking, up to date (click
 * to check again), a new version (click to see it), or a check that failed (click to try again).
 */
export function UpdateButton({ status, onCheck, onShow }: { status: UpdateStatus; onCheck: () => void; onShow: () => void }) {
  const s = status.state;
  const icon =
    s === "checking" ? (
      <LoaderIcon size={15} />
    ) : s === "available" ? (
      <DownloadIcon size={15} />
    ) : s === "current" ? (
      <OkBadge size={16} playOnMount />
    ) : s === "failed" ? (
      <BadgeAlertIcon size={15} />
    ) : (
      <RefreshCwIcon size={15} />
    );
  const label =
    s === "checking"
      ? "Checking for updates"
      : s === "available"
        ? `Update to ${status.update.version}`
        : s === "current"
          ? "You're up to date"
          : s === "failed"
            ? "Couldn't check. Try again"
            : "Check for updates";
  // Not `disabled` while checking: a disabled button drops focus, and About closes when focus
  // leaves it, under the pointer.
  return (
    <div className="update-check" aria-live="polite">
      <button
        type="button"
        className={`about-link update-btn is-${s}`}
        aria-disabled={s === "checking" || undefined}
        title={s === "current" ? "Check again" : undefined}
        onClick={s === "checking" ? undefined : s === "available" ? onShow : onCheck}
      >
        {icon}
        {label}
      </button>
    </div>
  );
}
