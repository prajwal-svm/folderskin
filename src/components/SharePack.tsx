import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  api,
  errorMessage,
  type GithubAccount,
  type Published,
  type PublishProgress,
  type SharedPack,
  type ShareProgress,
  type ShareStatus,
  type Skin,
} from "../lib/tauri";
import { isTauri } from "../lib/devMock";
import { cleanName } from "../lib/names";
import { creditDefaultProfile, LICENSES, licenseLabel, MAX_PACK_SKINS, PACK_TERMS_URL, PACK_TERMS_VERSION, packSlug, PACKS_GUIDE_URL, UPLOAD_URL } from "../lib/packs";
import { defaultProfile, loadProfiles, type LicenceProfile, type LicenseId } from "../lib/profiles";
import { handleFrom, isHandle, loadHandle, PICTURE_SOURCES, saveHandle, shareProgressLabel, type PictureSource } from "../lib/share";
import { MAX_PACK_TAGS, tagCounts, tagLabel } from "../lib/tags";
import { GithubAvatar } from "./GithubAvatar";
import { GithubConnect } from "./GithubConnect";
import { Modal } from "./Modal";
import { MySubmissions } from "./MySubmissions";
import { Select } from "./Select";
import { ShareVerify } from "./ShareVerify";
import { TagInput } from "./TagInput";
import { CheckIcon } from "./icons/check";
import { EarthIcon } from "./icons/earth";
import { ExternalLinkIcon } from "./icons/external-link";
import { FolderOpenIcon } from "./icons/folder-open";
import { GithubIcon } from "./icons/github";
import { LoaderIcon } from "./icons/loader";

/** Which way a pack goes: a pull request on GitHub, or FolderSkin's review queue for anyone without an account. */
type Route = "github" | "direct";

/** What FolderSkin is doing, in the words it says while doing it. */
function progressLabel(p: PublishProgress): string {
  switch (p.stage) {
    case "checking":
      return "Looking at what you can push to";
    case "forking":
      return "Making your own copy of FolderSkin";
    case "branching":
      return "Starting a branch for the pack";
    case "uploading":
      return `Sending pictures (${Math.min(p.done + 1, p.total)} of ${p.total})`;
    case "opening":
      return "Opening the pull request";
  }
}

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
 * and pick one of their licence profiles from Settings (or just a licence for this pack), then
 * press publish: FolderSkin signs them in to GitHub once, forks the repository if they can't push
 * to it, and opens the pull request for them.
 *
 * A pack holds up to {@link MAX_PACK_SKINS} skins, so the picker is a grid of ticks rather than a
 * single choice — sharing one skin and sharing twenty are the same dialog. Saving a folder is
 * still here for anyone who would rather do the GitHub part themselves.
 *
 * Without a GitHub account, the same pack goes to FolderSkin's review queue instead
 * (src-tauri/src/share.rs): the computer is verified once in the browser, under a name the packs
 * are credited to, and a person approves every pack before anyone else can see it. The dialog says
 * that, and what becomes public, before anything is sent; when this build has no service, or it
 * can't be reached, the choice says so rather than failing.
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

  const [account, setAccount] = useState<GithubAccount | null>(null);
  /** Who the pack is credited to. GitHub says who that is; nothing here types it. */
  const author = account?.login ?? "";
  /** Set while they are approving a code, and says what to do once they have. */
  const [connecting, setConnecting] = useState<null | "publish" | "sign-in">(null);
  const [progress, setProgress] = useState<PublishProgress | null>(null);
  const [published, setPublished] = useState<Published | null>(null);

  const [route, setRoute] = useState<Route>("github");
  /** What the sharing service says about this computer; asked for once the route is picked. */
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

  // Whether they are already signed in decides what the publish button does, so it is worth
  // knowing before they press it.
  useEffect(() => {
    let live = true;
    void api
      .githubAccount()
      .then((who) => {
        if (!live || !who) return;
        setAccount(who);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, []);

  // Asked only once someone picks the route: most people share through GitHub, and opening the
  // dialog shouldn't reach out to a service they aren't using.
  useEffect(() => {
    if (route !== "direct" || direct) return;
    let live = true;
    void api
      .shareStatus()
      .then((status) => live && setDirect(status))
      .catch((e) => live && setDirect({ available: false, reason: errorMessage(e), verified: false, handle: null, has_key: false }));
    return () => {
      live = false;
    };
  }, [route, direct]);

  const shown = filter ? yours.filter((s) => s.tags.includes(filter)) : yours;
  const chosen = yours.filter((s) => picked.includes(s.id));
  const allShown = shown.length > 0 && shown.every((s) => picked.includes(s.id));
  const clean = cleanName(name);
  // Short enough for the one line beside the buttons: the field itself says the rest.
  const packProblem =
    yours.length === 0
      ? "No skins of your own yet"
      : chosen.length === 0
        ? "Tick at least one skin"
        : chosen.length > MAX_PACK_SKINS
          ? `${chosen.length} skins is over the ${MAX_PACK_SKINS} a pack holds`
          : !clean || !packSlug(clean)
            ? "Give the pack a name"
            : packTags.length === 0
              ? "Add at least one tag"
              : null;
  const routeProblem =
    route === "github"
      ? !account
        ? "Connect to GitHub first"
        : null
      : !direct
        ? "Checking whether it's available"
        : !direct.available
          ? "Not available right now"
          : !source
            ? "Say where the pictures came from"
            : !direct.verified && !isHandle(handle)
              ? "Choose the name your packs show"
              : null;
  const problem = packProblem ?? routeProblem;
  /** The thing FolderSkin can't check for them, which is why they are asked rather than told. */
  const unconfirmed = !mine ? "Agree to the terms" : null;

  const pack = () => ({
    name: clean,
    license,
    tags: packTags,
    skinIds: chosen.map((s) => s.id),
    notes,
    termsVersion: PACK_TERMS_VERSION,
  });

  const toggle = (id: string) => setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p, id]));

  const takeAll = () =>
    setPicked((p) => (allShown ? p.filter((id) => !shown.some((s) => s.id === id)) : [...new Set([...p, ...shown.map((s) => s.id)])]));

  const narrow = (tag: string) => {
    setFilter(tag);
    if (tag && !name.trim()) setName(tagLabel(tag));
  };

  // A profile gives the pack its licence, and the name it credits becomes the name typed for a
  // computer not yet verified. GitHub, and a verified computer, say who a pack is credited to.
  const pickProfile = (id: string) => {
    const p = profiles.list.find((x) => x.id === id);
    if (!p) return;
    setProfileId(p.id);
    setLicense(p.license);
    if (p.author) setHandle(handleFrom(p.author));
  };
  /** Who the pack goes out credited to, once that's settled: the GitHub account, or the name this
   *  computer was verified under. */
  const creditedTo = route === "github" ? account?.login : direct?.verified ? direct.handle : null;
  const otherCredit = Boolean(profile.author && creditedTo && creditedTo.toLowerCase() !== profile.author.toLowerCase());

  // Crediting a different account means signing in as it, not signing out of this one: cancelling
  // half way leaves them where they were rather than logged out of a dialog they came here to use.
  const useAnother = () => setConnecting("sign-in");

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

  // `who` is the account just connected when connecting carries on and publishes: this render's
  // `account` is from before it.
  const publish = useCallback(async (who: GithubAccount | null = account) => {
    setBusy(true);
    setError(null);
    setProgress({ stage: "checking" });
    try {
      const out = await api.publishPack(pack(), setProgress);
      if (who) creditDefaultProfile(who.login);
      setPublished(out);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
      setProgress(null);
    }
    // `pack()` reads the current form, which is exactly what should be sent when it is pressed.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [license, notes, packTags, clean, chosen, account]);

  // Signing in from the author row only signs them in; signing in from the publish button carries
  // on and publishes. A ref keeps the callback itself stable, so the code on screen survives.
  const publishRef = useRef(publish);
  publishRef.current = publish;
  const wantedRef = useRef(connecting);
  wantedRef.current = connecting;
  const onConnected = useCallback((who: GithubAccount) => {
    const go = wantedRef.current === "publish";
    setAccount(who);
    setConnecting(null);
    if (go) void publishRef.current(who);
  }, []);

  // ---- without GitHub ----

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

  // Verifying carries on and sends, the way connecting to GitHub carries on and publishes. A ref
  // keeps the callback itself stable, so the check open in the browser isn't started over.
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
        sub="Packs you've shared without GitHub, and where each one is."
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

  // ---- it went up ----
  if (published) {
    return (
      <Modal
        narrow
        title="Your pack is on its way"
        sub={`Pull request #${published.number} is open on FolderSkin.`}
        onClose={onClose}
        footer={
          <>
            <button type="button" className="btn btn-ghost" onClick={onClose}>
              Done
            </button>
            <button type="button" className="btn btn-primary" onClick={() => void openUrl(published.url).catch(() => {})}>
              <ExternalLinkIcon />
              See the pull request
            </button>
          </>
        }
      >
        <p className="field-note">
          Once a maintainer merges it, it appears in everyone's Community view.{" "}
          <button type="button" className="link-btn" onClick={() => void openUrl(PACKS_GUIDE_URL).catch(() => {})}>
            How packs work <ExternalLinkIcon size={12} />
          </button>
        </p>
      </Modal>
    );
  }

  // ---- signing in ----
  if (connecting) {
    return (
      <Modal narrow title="Connect to GitHub" sub="So FolderSkin can open the pull request as you." onClose={onClose}>
        <GithubConnect onConnected={onConnected} onCancel={() => setConnecting(null)} />
      </Modal>
    );
  }

  // ---- saved a folder the old way ----
  if (saved) {
    const id = saved.split(/[\\/]/).pop() ?? packSlug(clean);
    return (
      <Modal
        narrow
        title="Your pack is ready"
        sub={`Saved as the folder ${id}.`}
        onClose={onClose}
        footer={
          <>
            <button type="button" className="btn btn-ghost" onClick={() => void revealItemInDir(saved).catch(() => {})}>
              <FolderOpenIcon size={15} />
              Show in {fileBrowser}
            </button>
            <button type="button" className="btn btn-primary" onClick={() => void openUrl(UPLOAD_URL).catch(() => {})}>
              <ExternalLinkIcon />
              Open GitHub
            </button>
          </>
        }
      >
        <ol className="share-steps">
          <li>Open GitHub and sign in. It makes you a copy of FolderSkin to add to.</li>
          <li>
            Drag the <strong>{id}</strong> folder onto the page.
          </li>
          <li>Choose Propose changes, then Create pull request.</li>
        </ol>
        <p className="field-note">
          Once it's checked and merged, everyone can add it from Community.{" "}
          <button type="button" className="link-btn" onClick={() => void openUrl(PACKS_GUIDE_URL).catch(() => {})}>
            How packs work <ExternalLinkIcon size={12} />
          </button>
        </p>
      </Modal>
    );
  }

  const stop = problem ?? unconfirmed;
  const verified = route === "direct" && direct?.verified === true;
  return (
    <Modal
      className="modal-share"
      title="Share a pack"
      sub={
        route === "github"
          ? "Packs are free. They live on GitHub, and anyone can add one to FolderSkin."
          : "Packs are free. Anyone can add one to FolderSkin once a person has reviewed it."
      }
      onClose={onClose}
      footer={
        <>
          {stop && !busy && (
            <span className="modal-reason" data-tip={stop} data-tip-overflow>
              {stop}
            </span>
          )}
          {route === "github" ? (
            <>
              <button type="button" className="btn btn-ghost" disabled={busy || !account} onClick={() => void save()}>
                Save a folder
              </button>
              <button
                type="button"
                className="btn btn-primary"
                disabled={Boolean(stop) || busy}
                aria-busy={busy}
                onClick={() => (account ? void publish() : setConnecting("publish"))}
              >
                {busy ? <LoaderIcon /> : <GithubIcon size={15} />}
                {busy ? "Publishing" : account ? "Publish" : "Connect and publish"}
              </button>
            </>
          ) : (
            <button
              type="button"
              className="btn btn-primary"
              disabled={Boolean(stop) || busy}
              aria-busy={busy}
              onClick={() => (verified ? void send() : verifyAndSend())}
            >
              {busy ? <LoaderIcon /> : <EarthIcon size={15} />}
              {busy ? "Sending" : verified ? "Send for review" : "Verify and send"}
            </button>
          )}
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
            <div className="share-author-head">
              <span className="field-label">Author</span>
              <div className="seg seg-sm share-route" role="radiogroup" aria-label="how to share">
                {(
                  [
                    ["github", "GitHub"],
                    ["direct", "Without GitHub"],
                  ] as const
                ).map(([id, label]) => (
                  <button
                    key={id}
                    type="button"
                    role="radio"
                    aria-checked={route === id}
                    className={route === id ? "seg-btn is-active" : "seg-btn"}
                    disabled={busy}
                    onClick={() => {
                      setRoute(id);
                      setError(null);
                    }}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>
            {route === "github" ? (
              account ? (
                <div className="gh-account">
                  <GithubAvatar account={account} />
                  <span className="gh-account-who">
                    <strong data-tip={account.login} data-tip-overflow>
                      {account.login}
                    </strong>
                  </span>
                  <button type="button" className="link-btn" onClick={useAnother}>
                    Use another
                  </button>
                </div>
              ) : (
                <button type="button" className="btn btn-ghost gh-pick" onClick={() => setConnecting("sign-in")}>
                  <GithubIcon size={15} />
                  Connect to GitHub
                </button>
              )
            ) : !direct ? (
              <p className="gh-waiting">
                <LoaderIcon />
                Checking whether sharing without GitHub is available
              </p>
            ) : !direct.available ? (
              <p className="field-note share-unavailable" role="status">
                {direct.reason ?? "Sharing without GitHub isn't available right now."}
              </p>
            ) : direct.verified && direct.handle ? (
              <>
                <div className="gh-account">
                  <span className="gh-avatar is-letter" aria-hidden="true">
                    {direct.handle.charAt(0).toUpperCase()}
                  </span>
                  <span className="gh-account-who">
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
                  The name your packs show: letters, digits and dashes. You'll confirm it in your browser once.{" "}
                  {direct.has_key ? null : (
                    <button type="button" className="link-btn" onClick={() => void loadKey()}>
                      Use a recovery file
                    </button>
                  )}
                </span>
              </>
            )}
            {otherCredit && (
              <span className="field-note">
                The {profile.name} profile credits {profile.author}.{" "}
                {route === "github"
                  ? "Through GitHub, packs are credited to the account connected here."
                  : "Packs sent from this computer are credited to the name it was verified under."}
              </span>
            )}
            {keyNote && <span className={keyNote.error ? "field-note is-error" : "field-note"}>{keyNote.text}</span>}
          </div>
          {route === "direct" && direct?.available && (
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

      {route === "direct" && direct?.available && (
        <ul className="share-direct" aria-label="how sharing without GitHub works">
          <li>
            <strong>Reviewed first.</strong> A person at FolderSkin looks at every pack. Nothing is public until it's approved.
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

      {progress && (
        <p className="gh-waiting">
          <LoaderIcon />
          {progressLabel(progress)}
        </p>
      )}
      {sending && (
        <p className="gh-waiting" role="status">
          <LoaderIcon />
          {shareProgressLabel(sending)}
        </p>
      )}
      {error && !busy && <p className="field-note is-error">{error}</p>}
    </Modal>
  );
}
