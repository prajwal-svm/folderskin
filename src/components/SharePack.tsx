import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type ExportedPack, type SharedPack, type ShareProgress, type ShareStatus, type Skin } from "../lib/tauri";
import { isTauri } from "../lib/devMock";
import { cleanName } from "../lib/names";
import { creditDefaultProfile, LICENSES, licenseLabel, MAX_PACK_SKINS } from "../lib/packs";
import { licenceOption } from "../lib/licences";
import { osOf } from "../lib/platform";
import { docsUrl, t as tNow, useT } from "../i18n";
import { Rich } from "../i18n/Rich";
import { defaultProfile, loadProfiles, type LicenceProfile, type LicenseId } from "../lib/profiles";
import { handleFrom, isHandle, loadHandle, PICTURE_SOURCES, saveHandle, scaledNote, shareProgressLabel, type PictureSource } from "../lib/share";
import { MAX_PACK_TAGS, tagCounts, tagLabel } from "../lib/tags";
import { Modal } from "./Modal";
import { MySubmissions } from "./MySubmissions";
import { Select } from "./Select";
import { ShareVerify } from "./ShareVerify";
import { TagInput } from "./TagInput";
import { CheckIcon } from "./icons/check";
import { EarthIcon } from "./icons/earth";
import { ExternalLinkIcon } from "./icons/external-link";
import { FolderOpenIcon } from "./icons/folder-open";
import { LoaderIcon } from "./icons/loader";
import { branded } from "./Brand";

const licenceOptions = () => LICENSES.map((l) => ({ value: l.id as LicenseId, label: licenceOption(l) }));

/** A profile as the choice of them names it: "Personal · credited to jane · CC0". */
function profileLabel(p: LicenceProfile): string {
  return [p.name, p.author && tNow("share.form.creditedTo", { author: p.author }), licenseLabel(p.license)].filter(Boolean).join(" · ");
}

/** Where the dialog starts: the skin they asked to share, all of theirs, or a tag small enough. */
function opening(yours: Skin[], only: Skin | undefined, tags: { tag: string; count: number }[]) {
  if (only) return { ids: [only.id], name: only.name, tags: only.tags.slice(0, MAX_PACK_TAGS), filter: "" };
  if (yours.length <= MAX_PACK_SKINS) return { ids: yours.map((s) => s.id), name: "", tags: [] as string[], filter: "" };
  const first = tags.find((t) => t.count <= MAX_PACK_SKINS);
  if (!first) return { ids: [] as string[], name: "", tags: [] as string[], filter: "" };
  return { ids: yours.filter((s) => s.tags.includes(first.tag)).map((s) => s.id), name: tagLabel(first.tag), tags: [first.tag], filter: first.tag };
}

/**
 * Shares the user's own skins with everyone. They choose which skins go in, name the pack, tag it
 * and pick one of their licence profiles from Settings (or just a licence for this pack), then send
 * it to FolderSkin's review queue (src-tauri/src/share.rs): the computer is verified once in the
 * browser, under a name the packs are credited to, and a person approves every pack before anyone
 * else can see it. The dialog says that, and what becomes public, before anything is sent; when
 * this build has no service, or it can't be reached, it says so rather than failing.
 *
 * A pack holds up to {@link MAX_PACK_SKINS} skins, so the picker is a grid of ticks rather than a
 * single choice: sharing one skin and sharing twenty are the same dialog. Saving the pack as a
 * folder is here too, for anyone who wants it as files, and works whether or not the service does.
 *
 * Either way every picture becomes a lossless WebP first, under a second each, so the line beside the
 * buttons counts them as they're ready; one too detailed for 1.5 MB at 1024 px is made smaller, and
 * the dialog that follows says which.
 */
export function SharePack({
  yours,
  only,
  os,
  onClose,
}: {
  yours: Skin[];
  /** Start with just this skin ticked. They can still tick more. */
  only?: Skin;
  /** The system, whose file browser the saved pack is shown in. */
  os: string;
  onClose: () => void;
}) {
  const t = useT();
  const tags = useMemo(() => tagCounts(yours), [yours]);
  const [start] = useState(() => opening(yours, only, tags));

  /** Which skins go in. A pack holds several, so this is a set of ids, not one choice. */
  const [picked, setPicked] = useState<string[]>(start.ids);
  /** Narrows the grid below. It does not decide what is in the pack. */
  const [filter, setFilter] = useState(start.filter);
  const [name, setName] = useState(start.name);
  const [packTags, setPackTags] = useState<string[]>(start.tags);

  /** The licence profiles kept in Settings. The dialog only reads them. */
  const [profiles] = useState(loadProfiles);
  const [profileId, setProfileId] = useState(profiles.defaultId);
  const profile = profiles.list.find((p) => p.id === profileId) ?? defaultProfile(profiles);
  /** The profile's licence to start with. Changed here, it's changed for this pack only. */
  const [license, setLicense] = useState<LicenseId>(() => defaultProfile(profiles).license);
  const [notes, setNotes] = useState("");
  const [mine, setMine] = useState(false);
  const [busy, setBusy] = useState(false);
  /** Set while the pack is being saved as a folder rather than sent. */
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<ExportedPack | null>(null);

  /** What the sharing service says about this computer. */
  const [direct, setDirect] = useState<ShareStatus | null>(null);
  /** The name packs will be credited to, typed before this computer is verified: the one typed
   *  last time, or else the name the default profile credits. */
  const [handle, setHandle] = useState(() => loadHandle() || handleFrom(defaultProfile(profiles).author));
  const [source, setSource] = useState<PictureSource | "">("");
  /** Set while the check runs in the browser; the pack is sent once it's passed. */
  const [verifying, setVerifying] = useState(false);
  const [sending, setSending] = useState<ShareProgress | null>(null);
  const [sent, setSent] = useState<SharedPack | null>(null);
  const [showMine, setShowMine] = useState(false);
  /** What happened to the recovery file, said under the name. */
  const [keyNote, setKeyNote] = useState<{ text: string; error?: boolean } | null>(null);

  // A build with no service answers without going over the network; one with a service asks it.
  useEffect(() => {
    let live = true;
    void api
      .shareStatus()
      .then((status) => live && setDirect(status))
      .catch((e) => live && setDirect({ available: false, reason: errorMessage(e), verified: false, handle: null, has_key: false, terms_version: null }));
    return () => {
      live = false;
    };
  }, []);

  const shown = filter ? yours.filter((s) => s.tags.includes(filter)) : yours;
  const chosen = yours.filter((s) => picked.includes(s.id));
  const allShown = shown.length > 0 && shown.every((s) => picked.includes(s.id));
  const clean = cleanName(name);
  const verified = direct?.verified === true && Boolean(direct.handle);
  /** The version of the pack terms it goes out under: the service's, while it can be used. */
  const terms = direct?.available ? direct.terms_version : null;
  /** Who the pack is credited to: the name this computer was verified under, or the one typed. */
  const author = verified && direct?.handle ? direct.handle : handle;
  // Short enough for the one line beside the buttons: the field itself says the rest.
  const packProblem =
    yours.length === 0
      ? t("share.problems.noSkins")
      : chosen.length === 0
        ? t("share.problems.tickOne")
        : chosen.length > MAX_PACK_SKINS
          ? t("share.problems.tooMany", { count: chosen.length, max: MAX_PACK_SKINS })
          : !clean
            ? t("share.problems.noName")
            : packTags.length === 0
              ? t("share.problems.noTag")
              : null;
  const nameProblem = !isHandle(author) ? t("share.problems.noHandle") : null;
  /** What stops the pack being sent, in the order it's worth fixing. */
  const sendProblem =
    packProblem ??
    (!direct ? t("share.problems.checking") : terms === null ? t("share.problems.unavailable") : !source ? t("share.problems.noSource") : nameProblem) ??
    // The thing FolderSkin can't check for them, which is why they are asked rather than told.
    (!mine ? t("share.problems.agree") : null);
  /** What stops it being saved as a folder: the pack and a name to credit, nothing about the service. */
  const saveProblem = packProblem ?? nameProblem;
  // While the service can't be used, the author field says why, and this line says what saving
  // a folder still needs.
  const reason = direct && !direct.available ? saveProblem : sendProblem;

  const toggle = (id: string) => setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p, id]));

  const takeAll = () =>
    setPicked((p) => (allShown ? p.filter((id) => !shown.some((s) => s.id === id)) : [...new Set([...p, ...shown.map((s) => s.id)])]));

  const narrow = (tag: string) => {
    setFilter(tag);
    if (tag && !name.trim()) setName(tagLabel(tag));
  };

  // A profile gives the pack its licence, and the name it credits becomes the name typed for a
  // computer not yet verified. A verified computer says who a pack is credited to.
  const pickProfile = (id: string) => {
    const p = profiles.list.find((x) => x.id === id);
    if (!p) return;
    setProfileId(p.id);
    setLicense(p.license);
    if (p.author) setHandle(handleFrom(p.author));
  };
  const otherCredit = Boolean(profile.author && verified && direct?.handle && direct.handle.toLowerCase() !== profile.author.toLowerCase());

  /** Writes the pack as a folder, for anyone who wants it as files. */
  const save = async () => {
    const folder = isTauri()
      ? await open({ directory: true, multiple: false, title: tNow("share.dialogs.saveWhere") }).catch(() => null)
      : "/Users/you/Desktop";
    if (typeof folder !== "string") return;
    setBusy(true);
    setSaving(true);
    setError(null);
    setSending({ stage: "encoding", done: 0, total: chosen.length });
    try {
      const out = await api.exportPack({ folder, name: clean, author, license, tags: packTags, skinIds: chosen.map((s) => s.id) }, (p) =>
        setSending({ stage: "encoding", ...p }),
      );
      creditDefaultProfile(author);
      setSaved(out);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
      setSaving(false);
      setSending(null);
    }
  };

  /** Sends the pack, recorded as agreed under version `termsVersion` of the pack terms. */
  const send = useCallback(async (termsVersion: number) => {
    setBusy(true);
    setError(null);
    setSending({ stage: "preparing" });
    try {
      const out = await api.shareSubmit(
        { name: clean, license, tags: packTags, skinIds: chosen.map((s) => s.id), notes, source, termsVersion },
        setSending,
      );
      setSent(out);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
      setSending(null);
    }
  }, [license, notes, packTags, clean, chosen, source]);

  // Verifying carries on and sends. A ref keeps the callback itself stable, so the check open in
  // the browser isn't started over.
  const sendRef = useRef(send);
  sendRef.current = send;
  const onVerified = useCallback((status: ShareStatus) => {
    setDirect(status);
    setVerifying(false);
    if (status.handle) saveHandle(status.handle);
    if (status.verified && status.available && status.terms_version !== null) void sendRef.current(status.terms_version);
  }, []);

  const verifyAndSend = () => {
    setError(null);
    saveHandle(handle);
    setVerifying(true);
  };

  /** Saves this computer's key, so the same name can share from another computer or after a reinstall. */
  const saveKey = async () => {
    const path = isTauri()
      ? await saveDialog({ title: tNow("share.dialogs.saveKey"), defaultPath: "folderskin-sharing-key.json", filters: [{ name: tNow("share.dialogs.keyFile"), extensions: ["json"] }] }).catch(() => null)
      : "/Users/you/Documents/folderskin-sharing-key.json";
    if (typeof path !== "string") return;
    try {
      await api.shareSaveKey(path);
      setKeyNote({ text: tNow("share.key.saved") });
    } catch (e) {
      setKeyNote({ text: errorMessage(e), error: true });
    }
  };

  /** Takes the key from a recovery file saved on another computer. */
  const loadKey = async () => {
    const path = isTauri()
      ? await open({ multiple: false, title: tNow("share.dialogs.chooseKey"), filters: [{ name: tNow("share.dialogs.keyFile"), extensions: ["json"] }] }).catch(() => null)
      : "/Users/you/Documents/folderskin-sharing-key.json";
    if (typeof path !== "string") return;
    try {
      setDirect(await api.shareLoadKey(path));
      setKeyNote(null);
    } catch (e) {
      setKeyNote({ text: errorMessage(e), error: true });
    }
  };

  // ---- sent for review ----
  if (sent) {
    const scaled = scaledNote(sent.scaled);
    return (
      <Modal
        narrow
        title={t("share.sent.title")}
        sub={t("share.sent.sub", { name: sent.name })}
        onClose={onClose}
        footer={
          <>
            <button type="button" className="btn btn-ghost" onClick={onClose}>
              {t("share.done")}
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => {
                setSent(null);
                setShowMine(true);
              }}
            >
              {t("share.sent.seeSubmissions")}
            </button>
          </>
        }
      >
        <p className="field-note">
          <Rich k="share.sent.note" vars={{ handle: direct?.handle ?? handle }} tags={{ b: (s) => <strong>{s}</strong> }} />
        </p>
        {scaled && <p className="field-note">{scaled}</p>}
      </Modal>
    );
  }

  // ---- verifying this computer ----
  if (verifying) {
    return (
      <Modal narrow title={t("share.verify.title")} sub={t("share.verify.sub")} onClose={onClose}>
        <ShareVerify handle={handle} onVerified={onVerified} onCancel={() => setVerifying(false)} />
      </Modal>
    );
  }

  // ---- what happened to the packs sent before ----
  if (showMine) {
    return (
      <Modal
        className="modal-share-subs"
        title={t("share.subs.title")}
        sub={t("share.subs.sub")}
        onClose={onClose}
        footer={
          <button type="button" className="btn btn-ghost" onClick={() => setShowMine(false)}>
            {t("share.subs.back")}
          </button>
        }
      >
        <MySubmissions />
      </Modal>
    );
  }

  // ---- saved as a folder ----
  if (saved) {
    const folder = saved.folder.split(/[\\/]/).pop() || saved.folder;
    const scaled = scaledNote(saved.scaled);
    return (
      <Modal
        narrow
        title={t("share.saved.title")}
        sub={t("share.saved.sub", { folder })}
        onClose={onClose}
        footer={
          <>
            <button type="button" className="btn btn-ghost" onClick={() => void revealItemInDir(saved.folder).catch(() => {})}>
              <FolderOpenIcon size={15} />
              {t(`common.showIn.${osOf(os)}`)}
            </button>
            <button type="button" className="btn btn-primary" onClick={onClose}>
              {t("share.done")}
            </button>
          </>
        }
      >
        <p className="field-note">
          {branded(t("share.saved.note"))}{" "}
          <button type="button" className="link-btn" onClick={() => void openUrl(docsUrl("packs")).catch(() => {})}>
            {t("share.saved.howPacksWork")} <ExternalLinkIcon size={12} />
          </button>
        </p>
        {scaled && <p className="field-note">{scaled}</p>}
      </Modal>
    );
  }

  return (
    <Modal
      className="modal-share"
      title={t("share.title")}
      sub={t("share.sub")}
      onClose={onClose}
      footer={
        <>
          {/* While it's saved or sent, the line beside the buttons says how far it has got, where it
              shows however far the form is scrolled: a big pack's pictures take a while together. */}
          {sending ? (
            <span className="modal-reason" role="status">
              {branded(shareProgressLabel(sending))}
            </span>
          ) : (
            reason &&
            !busy && (
              <span className="modal-reason" data-tip={reason} data-tip-overflow>
                {reason}
              </span>
            )
          )}
          <button type="button" className="btn btn-ghost" disabled={Boolean(saveProblem) || busy} aria-busy={saving} onClick={() => void save()}>
            {saving && <LoaderIcon />}
            {saving ? t("share.saving") : t("share.saveFolder")}
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={Boolean(sendProblem) || busy}
            aria-busy={busy && !saving}
            onClick={() => (verified && terms !== null ? void send(terms) : verifyAndSend())}
          >
            {busy && !saving ? <LoaderIcon /> : <EarthIcon size={15} />}
            {busy && !saving ? t("share.sending") : verified ? t("share.send") : t("share.verifyAndSend")}
          </button>
        </>
      }
    >
      <div className="share-grid">
        <div className="share-col">
          <div className="field share-pick-field">
            <div className="share-pick-head">
              <span className="field-label">{t("share.form.skins")}</span>
              <span className="share-count">{t("share.form.chosenOf", { chosen: chosen.length, total: yours.length })}</span>
              {shown.length > 1 && (
                <button type="button" className="link-btn" onClick={takeAll}>
                  {allShown ? t("share.form.clear") : t("share.form.selectAll")}
                </button>
              )}
            </div>
            {tags.length > 0 && yours.length > 1 && (
              <Select
                label={t("share.form.whichLabel")}
                className="is-field"
                value={filter}
                onChange={narrow}
                options={[{ value: "", label: t("share.form.allOfYours", { count: yours.length }) }, ...tags.map((c) => ({ value: c.tag, label: t("share.form.tagged", { tag: c.tag, count: c.count }) }))]}
              />
            )}
            <div className="share-pick-box">
              <div className="share-pick">
                {shown.map((s) => {
                  const on = picked.includes(s.id);
                  return (
                    <button
                      key={s.id}
                      type="button"
                      className={on ? "share-pick-one is-on" : "share-pick-one"}
                      aria-pressed={on}
                      data-tip={s.name}
                      data-tip-overflow
                      onClick={() => toggle(s.id)}
                    >
                      <img src={s.thumbnail} alt="" draggable={false} />
                      <span className="share-pick-name">{s.name}</span>
                      {on && (
                        <span className="share-pick-tick" aria-hidden="true">
                          <CheckIcon size={11} />
                        </span>
                      )}
                    </button>
                  );
                })}
                {shown.length === 0 && <p className="field-note">{t("share.form.nothingYet")}</p>}
              </div>
            </div>
          </div>
        </div>

        <div className="share-col">
          <label className="field">
            <span className="field-label">{t("share.form.packName")}</span>
            <input className="input" data-modal-focus value={name} maxLength={40} placeholder={t("share.form.packNamePlaceholder")} spellCheck={false} autoComplete="off" onChange={(e) => setName(e.target.value)} />
          </label>
          <div className="field">
            <span className="field-label">{t("share.form.tags")}</span>
            <TagInput value={packTags} onChange={setPackTags} suggestions={tags.map((c) => c.tag)} max={MAX_PACK_TAGS} label={t("share.form.tagsLabel")} />
            <span className="field-note">{t("share.form.tagsNote")}</span>
          </div>
          <div className="field">
            <span className="field-label">{t("share.form.profile")}</span>
            <Select
              label={t("share.form.profileLabel")}
              className="is-field"
              value={profile.id}
              onChange={pickProfile}
              options={profiles.list.map((p) => ({ value: p.id, label: profileLabel(p) }))}
            />
          </div>
          <div className="field">
            <span className="field-label">{t("share.form.licence")}</span>
            <Select label={t("share.form.licenceLabel")} className="is-field" value={license} onChange={setLicense} options={licenceOptions()} />
            {license !== profile.license && (
              <span className="field-note">{t("share.form.thisPackOnly", { profile: profile.name, licence: licenseLabel(profile.license) })}</span>
            )}
          </div>
          <div className="field">
            <span className="field-label">{t("share.form.author")}</span>
            {!direct ? (
              <p className="share-waiting">
                <LoaderIcon />
                {t("share.form.checking")}
              </p>
            ) : (
              <>
                {!direct.available && (
                  <p className="field-note share-unavailable" role="status">
                    {direct.reason ?? t("share.form.unavailable")}
                  </p>
                )}
                {verified && direct.handle ? (
                  <>
                    <div className="share-account">
                      <span className="share-avatar" aria-hidden="true">
                        {direct.handle.charAt(0).toUpperCase()}
                      </span>
                      <span className="share-account-who">
                        <strong data-tip={direct.handle} data-tip-overflow>
                          {direct.handle}
                        </strong>
                        <span>{t("share.form.verified")}</span>
                      </span>
                      <button type="button" className="link-btn" onClick={() => setShowMine(true)}>
                        {t("share.subs.title")}
                      </button>
                    </div>
                    <span className="field-note">
                      <Rich
                        k="share.key.saveNote"
                        tags={{
                          link: (s) => (
                            <button type="button" className="link-btn" onClick={() => void saveKey()}>
                              {s}
                            </button>
                          ),
                        }}
                      />
                    </span>
                  </>
                ) : (
                  <>
                    <input
                      className="input"
                      value={handle}
                      maxLength={39}
                      placeholder={t("share.form.handlePlaceholder")}
                      spellCheck={false}
                      autoComplete="off"
                      aria-label={t("share.form.handleLabel")}
                      onChange={(e) => setHandle(handleFrom(e.target.value))}
                      onBlur={() => setHandle((h) => h.replace(/-+$/, ""))}
                    />
                    <span className="field-note">
                      {direct.available ? t("common.twoSentences", { first: t("share.form.handleNote"), second: t("share.form.confirmOnce") }) : t("share.form.handleNote")}{" "}
                      {direct.available && !direct.has_key && (
                        <button type="button" className="link-btn" onClick={() => void loadKey()}>
                          {t("share.key.use")}
                        </button>
                      )}
                    </span>
                  </>
                )}
              </>
            )}
            {otherCredit && (
              <span className="field-note">{t("share.form.otherCredit", { profile: profile.name, author: profile.author })}</span>
            )}
            {keyNote && <span className={keyNote.error ? "field-note is-error" : "field-note"}>{keyNote.text}</span>}
          </div>
          {direct?.available && (
            <div className="field">
              <span className="field-label">{t("share.form.pictures")}</span>
              <Select<PictureSource | "">
                label={t("share.form.picturesLabel")}
                className="is-field"
                value={source}
                onChange={setSource}
                placeholder={t("share.form.picturesPlaceholder")}
                options={PICTURE_SOURCES.map((s) => ({ value: s.id, label: t(`share.sources.${s.id}`) }))}
              />
            </div>
          )}
          <label className="field">
            <span className="field-label">{t("share.form.credits")}</span>
            <textarea
              className="input share-notes"
              rows={2}
              maxLength={400}
              value={notes}
              placeholder={t("share.form.creditsPlaceholder")}
              onChange={(e) => setNotes(e.target.value)}
            />
          </label>
        </div>
      </div>

      {direct?.available && (
        <ul className="share-direct" aria-label={t("share.how.label")}>
          {(["reviewed", "public", "noAccount", "withdraw"] as const).map((point) => (
            <li key={point}>
              <Rich k={`share.how.${point}`} tags={{ b: (s) => <strong>{s}</strong> }} text={branded} />
            </li>
          ))}
        </ul>
      )}

      <div className="share-terms">
        <ul className="share-terms-list">
          <li>{t("share.terms.yours")}</li>
          <li>{t("share.terms.nothingHarmful")}</li>
          <li>{t("share.terms.nobodyElse")}</li>
          <li>{t("share.terms.maintainer")}</li>
        </ul>
        <label className="share-confirm">
          <input type="checkbox" checked={mine} onChange={(e) => setMine(e.target.checked)} />
          <span>
            {t("share.terms.agree")}{" "}
            <button type="button" className="link-btn" onClick={() => void openUrl(docsUrl("pack-terms")).catch(() => {})}>
              {t("share.terms.read")} <ExternalLinkIcon size={12} />
            </button>
          </span>
        </label>
      </div>

      {error && !busy && <p className="field-note is-error">{error}</p>}
    </Modal>
  );
}
