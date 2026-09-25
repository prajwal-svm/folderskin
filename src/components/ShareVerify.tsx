import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type ShareStatus } from "../lib/tauri";
import { ExternalLinkIcon } from "./icons/external-link";
import { LoaderIcon } from "./icons/loader";
import { branded } from "./Brand";
import { useT } from "../i18n";
import { Rich } from "../i18n/Rich";

/**
 * Verifying this computer for sharing packs. The check that keeps automated uploads out
 * (Cloudflare Turnstile) can't run inside the app, so FolderSkin signs a link with this computer's
 * key and opens it in the browser, then waits for the service to say the check was passed. It says
 * that it is waiting, and the page can be opened again if the browser was closed on it.
 */
export function ShareVerify({ handle, onVerified, onCancel }: { handle: string; onVerified: (status: ShareStatus) => void; onCancel: () => void }) {
  const t = useT();
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // The latest callback, so a parent that passes a new function each render doesn't start the
  // check over while the page is open in the browser.
  const verified = useRef(onVerified);
  verified.current = onVerified;

  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const page = await api.shareVerify(handle);
        if (!live) return;
        setUrl(page);
        void openUrl(page).catch(() => {});
        const status = await api.shareWait();
        if (live) verified.current(status);
      } catch (e) {
        if (live) setError(errorMessage(e));
      }
    })();
    return () => {
      live = false;
      // Whoever opened this has gone; stop asking the service about a check nobody is doing.
      void api.shareCancel().catch(() => {});
    };
  }, [handle]);

  if (error) {
    return (
      <div className="share-verify">
        <p className="field-note is-error">{error}</p>
        <button type="button" className="btn btn-ghost" onClick={onCancel}>
          {t("share.verify.back")}
        </button>
      </div>
    );
  }

  return (
    <div className="share-verify">
      <p className="share-verify-lead">{branded(t("share.verify.lead"))}</p>
      <p className="field-note">
        <Rich k="share.verify.creditedTo" vars={{ handle }} tags={{ b: (s) => <strong>{s}</strong> }} text={branded} />
      </p>
      <button type="button" className="btn btn-primary share-verify-open" disabled={!url} onClick={() => url && void openUrl(url).catch(() => {})}>
        <ExternalLinkIcon />
        {t("share.verify.openAgain")}
      </button>
      <p className="share-waiting" role="status">
        <LoaderIcon />
        {url ? t("share.verify.waiting") : t("share.verify.gettingReady")}
      </p>
      <button type="button" className="link-btn share-verify-cancel" onClick={onCancel}>
        {t("common.cancel")}
      </button>
    </div>
  );
}
