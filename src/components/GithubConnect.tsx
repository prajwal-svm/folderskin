import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type DeviceCode, type GithubAccount } from "../lib/tauri";
import { CheckIcon } from "./icons/check";
import { CopyIcon } from "./icons/copy";
import { ExternalLinkIcon } from "./icons/external-link";
import { LoaderIcon } from "./icons/loader";

/**
 * Connecting FolderSkin to GitHub, the way GitHub asks a program with no browser of its own to do
 * it: a short code, typed into a page, approved once.
 *
 * The code is the whole of the interaction, so it is the largest thing here, it can be copied with
 * one press, and opening the page copies it on the way — nobody should be typing eight characters
 * from one window into another. FolderSkin waits while they approve it and says so, because a
 * screen that looks finished but isn't is worse than a spinner.
 */
export function GithubConnect({ onConnected, onCancel }: { onConnected: (account: GithubAccount) => void; onCancel: () => void }) {
  const [code, setCode] = useState<DeviceCode | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const copiedFor = useRef(0);
  // The latest callback, so a parent that passes a new function each render doesn't re-run the
  // effect below and ask GitHub for a second code while they are typing in the first one.
  const connected = useRef(onConnected);
  connected.current = onConnected;

  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const asked = await api.githubConnect();
        if (!live) return;
        setCode(asked);
        const account = await api.githubWait();
        if (live) connected.current(account);
      } catch (e) {
        if (live) setError(errorMessage(e));
      }
    })();
    return () => {
      live = false;
      // Whoever opened this has gone; stop asking GitHub about a code nobody is looking at.
      void api.githubCancel().catch(() => {});
      window.clearTimeout(copiedFor.current);
    };
  }, []);

  const copy = async () => {
    if (!code) return;
    try {
      await navigator.clipboard.writeText(code.user_code);
      setCopied(true);
      window.clearTimeout(copiedFor.current);
      copiedFor.current = window.setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboards can be refused; the code is on screen to read either way.
    }
  };

  if (error) {
    return (
      <div className="gh-connect">
        <p className="field-note is-error">{error}</p>
        <button type="button" className="btn btn-ghost" onClick={onCancel}>
          Back
        </button>
      </div>
    );
  }

  if (!code) {
    return (
      <div className="gh-connect">
        <p className="gh-waiting">
          <LoaderIcon />
          Asking GitHub for a code
        </p>
      </div>
    );
  }

  return (
    <div className="gh-connect">
      <div className="gh-code">
        <span className="gh-code-value" aria-label={`your code is ${code.user_code.split("").join(" ")}`}>
          {code.user_code}
        </span>
        <button type="button" className="btn btn-ghost gh-copy" onClick={() => void copy()} aria-live="polite">
          {copied ? <CheckIcon size={15} /> : <CopyIcon size={15} />}
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <button
        type="button"
        className="btn btn-primary gh-open"
        onClick={() => {
          void copy();
          void openUrl(code.verification_uri).catch(() => {});
        }}
      >
        <ExternalLinkIcon />
        Open GitHub
      </button>
      <p className="gh-waiting">
        <LoaderIcon />
        Waiting for you to approve it
      </p>
      <p className="field-note">Public repositories only.</p>
      <button type="button" className="link-btn gh-cancel" onClick={onCancel}>
        Cancel
      </button>
    </div>
  );
}
