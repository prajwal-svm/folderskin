import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { canWithdraw, statusLabel } from "../lib/share";
import { PACK_TERMS_URL, licenseLabel } from "../lib/packs";
import { api, errorMessage, type MySubmission } from "../lib/tauri";
import { ExternalLinkIcon } from "./icons/external-link";
import { LoaderIcon } from "./icons/loader";

const when = (seconds: number) => new Date(seconds * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });

/** What withdrawing does to a pack, said before the second press. */
function withdrawNote(s: MySubmission): string {
  if (s.status !== "approved") return "It leaves the review queue.";
  // Only the maintainer can take a pack out of the community packs, so it isn't gone at once.
  if (s.pulled) return "It leaves the community for everyone once FolderSkin's maintainer has taken it out, usually within a few days. Copies people already have stay theirs.";
  return "It won't join the community packs.";
}

/**
 * The packs this computer has shared without GitHub, and where each one is: waiting, approved, or
 * turned down with the rule it broke and the maintainer's note. A pack can be taken back from here,
 * after a second press, since an approved one leaves the community for everyone. One already in the
 * community packs comes out once the maintainer has taken it out of them, which the row says.
 */
export function MySubmissions() {
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
      <p className="gh-waiting">
        <LoaderIcon />
        Asking FolderSkin's sharing service
      </p>
    );
  }
  if (list.length === 0) return <p className="field-note">Nothing yet. Packs you share without GitHub show up here.</p>;

  return (
    <>
      <ul className="share-subs" aria-label="your submissions">
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
                {s.pictures === 1 ? "1 picture" : `${s.pictures} pictures`} · {licenseLabel(s.license)} · sent {when(s.created_at)}
                {s.status === "approved" ? (s.pulled && s.pack_id ? ` · published as ${s.pack_id}` : " · joins the community packs soon") : ""}
              </p>
              {s.reasons.length > 0 && (
                <ul className="share-sub-reasons">
                  {s.reasons.map((r) => (
                    <li key={r.code}>
                      {r.message}{" "}
                      <button type="button" className="link-btn" onClick={() => void openUrl(PACK_TERMS_URL).catch(() => {})}>
                        Rule {r.term} <ExternalLinkIcon size={12} />
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              {s.note && <p className="share-sub-note">“{s.note}”</p>}
              {s.status === "withdrawn" && s.pulled && (
                <p className="share-sub-note">It was in the community packs, so FolderSkin's maintainer has been told to take it out. That can take a few days.</p>
              )}
              {canWithdraw(s.status) && (
                <div className="share-sub-actions">
                  {asking === s.id ? (
                    <>
                      <span className="field-note">{withdrawNote(s)}</span>
                      <button type="button" className="btn btn-ghost" onClick={() => setAsking(null)} disabled={busy === s.id}>
                        Keep it
                      </button>
                      <button type="button" className="btn btn-danger" aria-busy={busy === s.id} disabled={busy === s.id} onClick={() => void withdraw(s.id)}>
                        {busy === s.id ? <LoaderIcon /> : null}
                        Withdraw it
                      </button>
                    </>
                  ) : (
                    <button type="button" className="link-btn" onClick={() => setAsking(s.id)}>
                      Withdraw
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
