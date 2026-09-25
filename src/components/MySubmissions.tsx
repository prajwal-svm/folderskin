import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { canWithdraw, statusLabel } from "../lib/share";
import { licenseLabel } from "../lib/packs";
import { docsUrl, t, useT } from "../i18n";
import { formatDate } from "../i18n/format";
import { api, errorMessage, type MySubmission } from "../lib/tauri";
import { ExternalLinkIcon } from "./icons/external-link";
import { LoaderIcon } from "./icons/loader";
import { branded } from "./Brand";

const when = (seconds: number) => formatDate(seconds * 1000, { day: "numeric", month: "short", year: "numeric" });

/** What withdrawing does to a pack, said before the second press. */
function withdrawNote(s: MySubmission): string {
  if (s.status !== "approved") return t("share.subs.withdraw.queued");
  // Only the maintainer can take a pack out of the community packs, so it isn't gone at once.
  if (s.pulled) return t("share.subs.withdraw.published");
  return t("share.subs.withdraw.approved");
}

/**
 * The packs this computer has shared, and where each one is: waiting, approved, or
 * turned down with the rule it broke and the maintainer's note. A pack can be taken back from here,
 * after a second press, since an approved one leaves the community for everyone. One already in the
 * community packs comes out once the maintainer has taken it out of them, which the row says.
 */
export function MySubmissions() {
  const t = useT();
  const [list, setList] = useState<MySubmission[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** The pack whose Withdraw was pressed once, waiting for the press that means it. */
  const [asking, setAsking] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    void api
      .shareSubmissions()
      .then((found) => live && setList(found))
      .catch((e) => live && setError(errorMessage(e)));
    return () => {
      live = false;
    };
  }, []);

  const withdraw = async (id: string) => {
    setBusy(id);
    setError(null);
    try {
      await api.shareWithdraw(id);
      setList((l) => l?.map((s) => (s.id === id ? { ...s, status: "withdrawn", pack_id: null } : s)) ?? l);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(null);
      setAsking(null);
    }
  };

  if (!list) {
    return error ? (
      <p className="field-note is-error">{error}</p>
    ) : (
      <p className="share-waiting">
        <LoaderIcon />
        {branded(t("share.subs.asking"))}
      </p>
    );
  }
  if (list.length === 0) return <p className="field-note">{t("share.subs.none")}</p>;

  return (
    <>
      <ul className="share-subs" aria-label={t("share.subs.label")}>
        {list.map((s) => {
          const status = statusLabel(s.status);
          return (
            <li key={s.id} className="share-sub">
              <div className="share-sub-head">
                <strong className="share-sub-name" data-tip={s.name} data-tip-overflow>
                  {s.name}
                </strong>
                <span className={status.tone === "plain" ? "chip" : `chip chip-${status.tone}`}>{status.label}</span>
              </div>
              <p className="share-sub-meta">
                {t("share.subs.pictures", { count: s.pictures })} · {licenseLabel(s.license)} · {t("share.subs.sent", { date: when(s.created_at) })}
                {s.status === "approved" ? ` · ${s.pulled && s.pack_id ? t("share.subs.publishedAs", { id: s.pack_id }) : t("share.subs.joinsSoon")}` : ""}
              </p>
              {s.reasons.length > 0 && (
                <ul className="share-sub-reasons">
                  {s.reasons.map((r) => (
                    <li key={r.code}>
                      {r.message}{" "}
                      <button type="button" className="link-btn" onClick={() => void openUrl(docsUrl("pack-terms")).catch(() => {})}>
                        {t("share.subs.rule", { term: String(r.term) })} <ExternalLinkIcon size={12} />
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              {s.note && <p className="share-sub-note">{t("share.subs.quoted", { note: s.note })}</p>}
              {s.status === "withdrawn" && s.pulled && <p className="share-sub-note">{branded(t("share.subs.takingOut"))}</p>}
              {canWithdraw(s.status) && (
                <div className="share-sub-actions">
                  {asking === s.id ? (
                    <>
                      <span className="field-note">{branded(withdrawNote(s))}</span>
                      <button type="button" className="btn btn-ghost" onClick={() => setAsking(null)} disabled={busy === s.id}>
                        {t("share.subs.keep")}
                      </button>
                      <button type="button" className="btn btn-danger" aria-busy={busy === s.id} disabled={busy === s.id} onClick={() => void withdraw(s.id)}>
                        {busy === s.id ? <LoaderIcon /> : null}
                        {t("share.subs.withdrawIt")}
                      </button>
                    </>
                  ) : (
                    <button type="button" className="link-btn" onClick={() => setAsking(s.id)}>
                      {t("share.subs.withdraw.button")}
                    </button>
                  )}
                </div>
              )}
            </li>
          );
        })}
      </ul>
      {error && <p className="field-note is-error">{error}</p>}
    </>
  );
}
