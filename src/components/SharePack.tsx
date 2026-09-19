import { useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type Skin } from "../lib/tauri";
import { isTauri } from "../lib/devMock";
import { cleanName } from "../lib/names";
import { isGithubUser, LICENSES, loadSharingPrefs, MAX_PACK_SKINS, packSlug, PACKS_GUIDE_URL, saveSharingPrefs, UPLOAD_URL } from "../lib/packs";
import { MAX_PACK_TAGS, tagCounts, tagLabel } from "../lib/tags";
import { Modal } from "./Modal";
import { TagInput } from "./TagInput";
import { ExternalLinkIcon } from "./icons/external-link";
import { FolderOpenIcon } from "./icons/folder-open";
import { LoaderIcon } from "./icons/loader";

/**
 * Shares the user's own skins with everyone, in two steps. First: which skins (a whole set, or
 * just one, which goes up as a pack of one), the pack's name and tags, their GitHub name and a
 * licence. Then the folder is saved, already in the shape the pull-request checks want, and the
 * dialog says how to put it on GitHub.
 */
export function SharePack({
  yours,
  only,
  fileBrowser,
  onClose,
}: {
  yours: Skin[];
  /** Share just this skin. */
  only?: Skin;
  fileBrowser: string;
  onClose: () => void;
}) {
  const tags = useMemo(() => tagCounts(yours), [yours]);
  const [which, setWhich] = useState(() =>
    only || yours.length <= MAX_PACK_SKINS ? "" : (tags.find((t) => t.count <= MAX_PACK_SKINS)?.tag ?? ""),
  );
  const [name, setName] = useState(only ? only.name : which ? tagLabel(which) : "");
  const [packTags, setPackTags] = useState<string[]>(only ? only.tags.slice(0, MAX_PACK_TAGS) : which ? [which] : []);
  const [author, setAuthor] = useState(() => loadSharingPrefs().author);
  const [license, setLicense] = useState<string>(() => loadSharingPrefs().license);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);

  const chosen = only ? [only] : which ? yours.filter((s) => s.tags.includes(which)) : yours;
  const clean = cleanName(name);
  const problem =
    yours.length === 0
      ? "You have no skins of your own yet. Add a picture or make one with AI first."
      : chosen.length > MAX_PACK_SKINS
        ? `That's ${chosen.length} skins, and a pack holds ${MAX_PACK_SKINS}. Pick a tag to narrow it down.`
        : !clean || !packSlug(clean)
          ? "Give the pack a name with letters or digits in it."
          : packTags.length === 0
            ? "Add at least one tag. The first one names the pack in everyone's filters."
            : !isGithubUser(author.trim())
              ? "Add your GitHub user name, so people can credit you."
              : null;

  const pick = (tag: string) => {
    setWhich(tag);
    if (tag) {
      setPackTags((t) => [tag, ...t.filter((x) => x !== tag)].slice(0, MAX_PACK_TAGS));
      if (!name.trim()) setName(tagLabel(tag));
    }
  };

  const save = async () => {
    const folder = isTauri()
      ? await open({ directory: true, multiple: false, title: "Choose where to save the pack" }).catch(() => null)
      : "/Users/you/Desktop";
    if (typeof folder !== "string") return;
    setBusy(true);
    setError(null);
    try {
      const path = await api.exportPack({ folder, name: clean, author: author.trim(), license, tags: packTags, skinIds: chosen.map((s) => s.id) });
      saveSharingPrefs({ author: author.trim(), license });
      setSaved(path);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

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
            How packs work
          </button>
        </p>
      </Modal>
    );
  }

  return (
    <Modal
      narrow
      title={only ? `Share "${only.name}"` : "Share your skins"}
      sub={
        only
          ? "It goes up as a pack of one, free for anyone to add to FolderSkin."
          : "Packs are free. They live on GitHub, and anyone can add one to FolderSkin."
      }
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn btn-primary" disabled={Boolean(problem) || busy} aria-busy={busy} onClick={() => void save()}>
            {busy && <LoaderIcon />}
            {busy ? "Saving…" : "Save pack…"}
          </button>
        </>
      }
    >
      {only ? (
        <div className="share-one">
          <img src={only.thumbnail} alt="" draggable={false} />
          <span>{only.name}</span>
        </div>
      ) : (
      <label className="field">
        <span className="field-label">Skins</span>
        <select className="input" value={which} onChange={(e) => pick(e.target.value)}>
          <option value="">All of yours ({yours.length})</option>
          {tags.map((t) => (
            <option key={t.tag} value={t.tag}>
              Tagged {t.tag} ({t.count})
            </option>
          ))}
        </select>
        {chosen.length > 0 && (
          <span className="share-thumbs" aria-hidden="true">
            {chosen.slice(0, 8).map((s) => (
              <img key={s.id} src={s.thumbnail} alt="" draggable={false} />
            ))}
            {chosen.length > 8 && <span className="share-more">+{chosen.length - 8}</span>}
          </span>
        )}
      </label>
      )}
      <label className="field">
        <span className="field-label">Pack name</span>
        <input className="input" value={name} maxLength={40} placeholder="Neon nights" spellCheck={false} autoComplete="off" onChange={(e) => setName(e.target.value)} />
      </label>
      <div className="field">
        <span className="field-label">Tags</span>
        <TagInput value={packTags} onChange={setPackTags} suggestions={tags.map((t) => t.tag)} max={MAX_PACK_TAGS} label="add a tag for the pack" />
      </div>
      <label className="field">
        <span className="field-label">Your GitHub user name</span>
        <input className="input" value={author} maxLength={39} placeholder="octocat" spellCheck={false} autoComplete="off" onChange={(e) => setAuthor(e.target.value)} />
      </label>
      <label className="field">
        <span className="field-label">Licence</span>
        <select className="input" value={license} onChange={(e) => setLicense(e.target.value)}>
          {LICENSES.map((l) => (
            <option key={l.id} value={l.id}>
              {l.label}: {l.note}
            </option>
          ))}
        </select>
      </label>
      {(error ?? problem) && <p className={error ? "field-note is-error" : "field-note"}>{error ?? problem}</p>}
    </Modal>
  );
}
