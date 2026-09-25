import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type SharedPack, type ShareProgress, type ShareStatus, type Skin } from "../lib/tauri";
import { isTauri } from "../lib/devMock";
import { cleanName } from "../lib/names";
import { creditDefaultProfile, LICENSES, licenseLabel, MAX_PACK_SKINS, PACK_TERMS_URL, PACK_TERMS_VERSION, PACKS_GUIDE_URL } from "../lib/packs";
import { defaultProfile, loadProfiles, type LicenceProfile, type LicenseId } from "../lib/profiles";
import { handleFrom, isHandle, loadHandle, PICTURE_SOURCES, saveHandle, shareProgressLabel, type PictureSource } from "../lib/share";
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
import { Brand, branded } from "./Brand";

const LICENCE_OPTIONS = LICENSES.map((l) => ({ value: l.id as LicenseId, label: `${l.label}: ${l.note}` }));

/** A profile as the choice of them names it: "Personal · credited to jane · CC0". */
function profileLabel(p: LicenceProfile): string {
  return [p.name, p.author && `credited to ${p.author}`, licenseLabel(p.license)].filter(Boolean).join(" · ");
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
 */
export function SharePack({
  yours,
  only,
  fileBrowser,
  onClose,
}: {
  yours: Skin[];
  /** Start with just this skin ticked. They can still tick more. */
  only?: Skin;
  fileBrowser: string;
  onClose: () => void;
}) {
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
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);

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
      .catch((e) => live && setDirect({ available: false, reason: errorMessage(e), verified: false, handle: null, has_key: false }));
    return () => {
      live = false;
    };
  }, []);

  const shown = filter ? yours.filter((s) => s.tags.includes(filter)) : yours;
  const chosen = yours.filter((s) => picked.includes(s.id));
  const allShown = shown.length > 0 && shown.every((s) => picked.includes(s.id));
  const clean = cleanName(name);
  const verified = direct?.verified === true && Boolean(direct.handle);
  /** Who the pack is credited to: the name this computer was verified under, or the one typed. */
  const author = verified && direct?.handle ? direct.handle : handle;
  // Short enough for the one line beside the buttons: the field itself says the rest.
  const packProblem =
    yours.length === 0
      ? "No skins of your own yet"
      : chosen.length === 0
        ? "Tick at least one skin"
        : chosen.length > MAX_PACK_SKINS
          ? `${chosen.length} skins is over the ${MAX_PACK_SKINS} a pack holds`
          : !clean
            ? "Give the pack a name"
            : packTags.length === 0
              ? "Add at least one tag"
              : null;
  const nameProblem = !isHandle(author) ? "Choose the name your packs show" : null;
  /** What stops the pack being sent, in the order it's worth fixing. */
  const sendProblem =
    packProblem ??
    (!direct ? "Checking whether it's available" : !direct.available ? "Not available right now" : !source ? "Say where the pictures came from" : nameProblem) ??
    // The thing FolderSkin can't check for them, which is why they are asked rather than told.
    (!mine ? "Agree to the terms" : null);
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
      ? await open({ directory: true, multiple: false, title: "Choose where to save the pack" }).catch(() => null)
      : "/Users/you/Desktop";
    if (typeof folder !== "string") return;
    setBusy(true);
    setError(null);
    try {
      const path = await api.exportPack({ folder, name: clean, author, license, tags: packTags, skinIds: chosen.map((s) => s.id) });
      creditDefaultProfile(author);
      setSaved(path);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const send = useCallback(async () => {
    setBusy(true);
    setError(null);
    setSending({ stage: "preparing" });
    try {
      const out = await api.shareSubmit(
        { name: clean, license, tags: packTags, skinIds: chosen.map((s) => s.id), notes, source, termsVersion: PACK_TERMS_VERSION },
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
    if (status.verified) void sendRef.current();
  }, []);

  const verifyAndSend = () => {
    setError(null);
    saveHandle(handle);
    setVerifying(true);
  };

  /** Saves this computer's key, so the same name can share from another computer or after a reinstall. */
  const saveKey = async () => {
    const path = isTauri()
      ? await saveDialog({ title: "Save your recovery file", defaultPath: "folderskin-sharing-key.json", filters: [{ name: "Recovery file", extensions: ["json"] }] }).catch(() => null)
      : "/Users/you/Documents/folderskin-sharing-key.json";
    if (typeof path !== "string") return;
    try {
      await api.shareSaveKey(path);
      setKeyNote({ text: "Saved. Keep it private: anyone who has it can share as you." });
    } catch (e) {
      setKeyNote({ text: errorMessage(e), error: true });
    }
  };

  /** Takes the key from a recovery file saved on another computer. */
  const loadKey = async () => {
    const path = isTauri()
      ? await open({ multiple: false, title: "Choose your recovery file", filters: [{ name: "Recovery file", extensions: ["json"] }] }).catch(() => null)
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
    return (
      <Modal
        narrow
        title="Your pack is waiting for review"
        sub={`${sent.name} is in FolderSkin's review queue.`}
        onClose={onClose}
        footer={
          <>
            <button type="button" className="btn btn-ghost" onClick={onClose}>
              Done
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => {
                setSent(null);
                setShowMine(true);
              }}
            >
              See your submissions
            </button>
          </>
        }
      >
        <p className="field-note">
          A person looks at every pack before anyone else can see it. Once it's approved it joins the Community view for everyone, credited to{" "}
          <strong>{direct?.handle ?? handle}</strong>. If it's turned down, Your submissions says why.
        </p>
      </Modal>
    );
  }

  // ---- verifying this computer ----
  if (verifying) {
    return (
      <Modal narrow title="Verify this computer" sub="So only people, not scripts, can send packs for review." onClose={onClose}>
        <ShareVerify handle={handle} onVerified={onVerified} onCancel={() => setVerifying(false)} />
      </Modal>
    );
  }

  // ---- what happened to the packs sent before ----
  if (showMine) {
    return (
      <Modal
        className="modal-share-subs"
        title="Your submissions"
        sub="Packs you've shared, and where each one is."
        onClose={onClose}
        footer={
          <button type="button" className="btn btn-ghost" onClick={() => setShowMine(false)}>
            Back to sharing
          </button>
        }
      >
        <MySubmissions />
      </Modal>
    );
  }

  // ---- saved as a folder ----
  if (saved) {
    const folder = saved.split(/[\\/]/).pop() || saved;
    return (
      <Modal
        narrow
        title="Your pack is ready"
        sub={`Saved as the folder ${folder}.`}
        onClose={onClose}
        footer={
          <>
            <button type="button" className="btn btn-ghost" onClick={() => void revealItemInDir(saved).catch(() => {})}>
              <FolderOpenIcon size={15} />
              Show in {fileBrowser}
            </button>
            <button type="button" className="btn btn-primary" onClick={onClose}>
              Done
            </button>
          </>
        }
      >
        <p className="field-note">
          It holds the pictures and the pack.json that describes them. Anyone can add it to <Brand /> with Add from a folder in Community,
          and it stays on this computer until you share it.{" "}
          <button type="button" className="link-btn" onClick={() => void openUrl(PACKS_GUIDE_URL).catch(() => {})}>
            How packs work <ExternalLinkIcon size={12} />
          </button>
        </p>
      </Modal>
    );
  }

  return (
    <Modal
      className="modal-share"
      title="Share a pack"
      sub="Packs are free. Anyone can add one to FolderSkin once a person has reviewed it."
      onClose={onClose}
      footer={
        <>
          {reason && !busy && (
            <span className="modal-reason" data-tip={reason} data-tip-overflow>
              {reason}
            </span>
          )}
          <button type="button" className="btn btn-ghost" disabled={Boolean(saveProblem) || busy} onClick={() => void save()}>
            Save a folder
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={Boolean(sendProblem) || busy}
            aria-busy={busy}
            onClick={() => (verified ? void send() : verifyAndSend())}
          >
            {busy ? <LoaderIcon /> : <EarthIcon size={15} />}
            {busy ? "Sending" : verified ? "Send for review" : "Verify and send"}
          </button>
        </>
      }
    >
      <div className="share-grid">
        <div className="share-col">
          <div className="field share-pick-field">
            <div className="share-pick-head">
              <span className="field-label">Skins</span>
              <span className="share-count">
                {chosen.length} of {yours.length}
              </span>
              {shown.length > 1 && (
                <button type="button" className="link-btn" onClick={takeAll}>
                  {allShown ? "Clear" : "Select all"}
                </button>
              )}
            </div>
            {tags.length > 0 && yours.length > 1 && (
              <Select
                label="which skins to show"
                className="is-field"
                value={filter}
                onChange={narrow}
                options={[{ value: "", label: `All of yours (${yours.length})` }, ...tags.map((t) => ({ value: t.tag, label: `Tagged ${t.tag} (${t.count})` }))]}
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
                {shown.length === 0 && <p className="field-note">Nothing here yet.</p>}
              </div>
            </div>
          </div>
        </div>

        <div className="share-col">
          <label className="field">
            <span className="field-label">Pack name</span>
            <input className="input" data-modal-focus value={name} maxLength={40} placeholder="Neon nights" spellCheck={false} autoComplete="off" onChange={(e) => setName(e.target.value)} />
          </label>
          <div className="field">
            <span className="field-label">Tags</span>
            <TagInput value={packTags} onChange={setPackTags} suggestions={tags.map((t) => t.tag)} max={MAX_PACK_TAGS} label="add a tag for the pack" />
            <span className="field-note">The first one names the pack in everyone's filters.</span>
          </div>
          <div className="field">
            <span className="field-label">Profile</span>
            <Select
              label="profile"
              className="is-field"
              value={profile.id}
              onChange={pickProfile}
              options={profiles.list.map((p) => ({ value: p.id, label: profileLabel(p) }))}
            />
          </div>
          <div className="field">
            <span className="field-label">Licence</span>
            <Select label="licence" className="is-field" value={license} onChange={setLicense} options={LICENCE_OPTIONS} />
            {license !== profile.license && (
              <span className="field-note">
                For this pack only: the {profile.name} profile stays {licenseLabel(profile.license)}.
              </span>
            )}
          </div>
          <div className="field">
            <span className="field-label">Author</span>
            {!direct ? (
              <p className="share-waiting">
                <LoaderIcon />
                Checking whether sharing is available
              </p>
            ) : (
              <>
                {!direct.available && (
                  <p className="field-note share-unavailable" role="status">
                    {direct.reason ?? "Sharing isn't available right now."}
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
                        <span>Verified on this computer</span>
                      </span>
                      <button type="button" className="link-btn" onClick={() => setShowMine(true)}>
                        Your submissions
                      </button>
                    </div>
                    <span className="field-note">
                      <button type="button" className="link-btn" onClick={() => void saveKey()}>
                        Save a recovery file
                      </button>{" "}
                      to share under this name from another computer.
                    </span>
                  </>
                ) : (
                  <>
                    <input
                      className="input"
                      value={handle}
                      maxLength={39}
                      placeholder="your-name"
                      spellCheck={false}
                      autoComplete="off"
                      aria-label="the name your packs show"
                      onChange={(e) => setHandle(handleFrom(e.target.value))}
                      onBlur={() => setHandle((h) => h.replace(/-+$/, ""))}
                    />
                    <span className="field-note">
                      The name your packs show: letters, digits and dashes.
                      {direct.available && " You'll confirm it in your browser once."}{" "}
                      {direct.available && !direct.has_key && (
                        <button type="button" className="link-btn" onClick={() => void loadKey()}>
                          Use a recovery file
                        </button>
                      )}
                    </span>
                  </>
                )}
              </>
            )}
            {otherCredit && (
              <span className="field-note">
                The {profile.name} profile credits {profile.author}. Packs sent from this computer are credited to the name it was verified under.
              </span>
            )}
            {keyNote && <span className={keyNote.error ? "field-note is-error" : "field-note"}>{keyNote.text}</span>}
          </div>
          {direct?.available && (
            <div className="field">
              <span className="field-label">The pictures</span>
              <Select<PictureSource | "">
                label="the pictures"
                className="is-field"
                value={source}
                onChange={setSource}
                placeholder="Where did they come from?"
                options={PICTURE_SOURCES.map((s) => ({ value: s.id, label: s.label }))}
              />
            </div>
          )}
          <label className="field">
            <span className="field-label">Credits</span>
            <textarea
              className="input share-notes"
              rows={2}
              maxLength={400}
              value={notes}
              placeholder="Base photo by Jane Doe, CC0"
              onChange={(e) => setNotes(e.target.value)}
            />
          </label>
        </div>
      </div>

      {direct?.available && (
        <ul className="share-direct" aria-label="how sharing works">
          <li>
            <strong>Reviewed first.</strong> A person at <Brand /> looks at every pack. Nothing is public until it&apos;s approved.
          </li>
          <li>
            <strong>Public once approved.</strong> The pictures, the pack's name and tags, and your name go to everyone under the licence
            you chose, and a licence can't be taken back from copies people already have.
          </li>
          <li>
            <strong>No account.</strong> The service keeps this computer's key and your name. Your network address is only ever kept
            scrambled.
          </li>
          <li>
            <strong>Yours to withdraw.</strong> Your submissions shows where each pack is, says why if one is turned down, and takes one
            back.
          </li>
        </ul>
      )}

      <div className="share-terms">
        <ul className="share-terms-list">
          <li>The pictures are yours, or CC0 and you've checked. Not taken from anywhere.</li>
          <li>Nothing sexual, hateful, gory, or about self-harm. Nothing involving a child.</li>
          <li>Nobody else's logo, characters or likeness without their say-so.</li>
          <li>A maintainer can decline a pack, or remove it later.</li>
        </ul>
        <label className="share-confirm">
          <input type="checkbox" checked={mine} onChange={(e) => setMine(e.target.checked)} />
          <span>
            I've read the pack terms and this pack follows them.{" "}
            <button type="button" className="link-btn" onClick={() => void openUrl(PACK_TERMS_URL).catch(() => {})}>
              Read them <ExternalLinkIcon size={12} />
            </button>
          </span>
        </label>
      </div>

      {sending && (
        <p className="share-waiting" role="status">
          <LoaderIcon />
          {branded(shareProgressLabel(sending))}
        </p>
      )}
      {error && !busy && <p className="field-note is-error">{error}</p>}
    </Modal>
  );
}
