import { useCallback, useEffect, useState } from "react";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type AiCatalogue } from "../lib/tauri";
import { prettyPath } from "../lib/files";
import { isGithubUser, LICENSES, loadSharingPrefs, PACKS_GUIDE_URL, REPO_URL, saveSharingPrefs, type SharingPrefs } from "../lib/packs";
import type { ThemePref } from "../state/theme";
import type { ToastTone } from "../hooks/useToasts";
import { Modal } from "./Modal";
import { ProviderKeys } from "./ProviderKeys";
import { FolderOpenIcon } from "./icons/folder-open";
import { LoaderIcon } from "./icons/loader";

export type SettingsTab = "general" | "ai" | "sharing" | "about";

const TABS: { id: SettingsTab; label: string }[] = [
  { id: "general", label: "General" },
  { id: "ai", label: "AI keys" },
  { id: "sharing", label: "Sharing" },
  { id: "about", label: "About" },
];

const THEMES: { id: ThemePref; label: string }[] = [
  { id: "system", label: "Match the system" },
  { id: "light", label: "Light" },
  { id: "dark", label: "Dark" },
];

type Toast = (text: string, opts?: { tone?: ToastTone }) => void;

/**
 * Every setting in one dialog: how FolderSkin looks and where it keeps your skins, the AI keys,
 * what sharing a pack fills in, and about the app. The sidebar's dark mode switch and the
 * studio's key dialog stay where they are and change the same settings.
 */
export function Settings({
  tab: first = "general",
  themePref,
  onThemePref,
  fileBrowser,
  savedCount,
  note,
  onKeysChanged,
  onClose,
  toast,
}: {
  tab?: SettingsTab;
  themePref: ThemePref;
  onThemePref: (pref: ThemePref) => void;
  fileBrowser: string;
  /** Skins saved on this computer: your own and community ones. */
  savedCount: number;
  /** How this OS keeps a folder's icon. */
  note: string;
  /** A key was saved or removed, so the studio reloads its providers. */
  onKeysChanged: () => void;
  onClose: () => void;
  toast: Toast;
}) {
  const [tab, setTab] = useState<SettingsTab>(first);
  return (
    <Modal
      wide
      title="Settings"
      onClose={onClose}
      footer={
        <button type="button" className="btn btn-primary" onClick={onClose}>
          Done
        </button>
      }
    >
      <div className="settings-tabs" role="tablist" aria-label="settings">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={t.id === tab}
            className={t.id === tab ? "settings-tab is-active" : "settings-tab"}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>
      <div className="settings-page" role="tabpanel" key={tab}>
        {tab === "general" && <General themePref={themePref} onThemePref={onThemePref} fileBrowser={fileBrowser} savedCount={savedCount} />}
        {tab === "ai" && <AiKeys onKeysChanged={onKeysChanged} toast={toast} />}
        {tab === "sharing" && <Sharing />}
        {tab === "about" && <About note={note} />}
      </div>
    </Modal>
  );
}

function General({
  themePref,
  onThemePref,
  fileBrowser,
  savedCount,
}: {
  themePref: ThemePref;
  onThemePref: (pref: ThemePref) => void;
  fileBrowser: string;
  savedCount: number;
}) {
  const [folder, setFolder] = useState<string | null>(null);
  const [folderError, setFolderError] = useState<string | null>(null);
  useEffect(() => {
    api
      .skinsFolder()
      .then(setFolder)
      .catch((e) => setFolderError(errorMessage(e)));
  }, []);

  return (
    <>
      <div className="field">
        <span className="field-label">Appearance</span>
        <div className="choice" role="radiogroup" aria-label="appearance">
          {THEMES.map((t) => (
            <button
              key={t.id}
              type="button"
              role="radio"
              aria-checked={t.id === themePref}
              className={t.id === themePref ? "choice-btn is-active" : "choice-btn"}
              onClick={() => onThemePref(t.id)}
            >
              {t.label}
            </button>
          ))}
        </div>
      </div>
      <div className="field">
        <span className="field-label">Your skins</span>
        <p className="settings-line">
          {savedCount === 1 ? "1 skin is saved" : `${savedCount} skins are saved`}
          {folder ? (
            <>
              {" "}
              in <code className="settings-path">{prettyPath(folder)}</code>
            </>
          ) : null}
          .
        </p>
        {folder && (
          <button type="button" className="btn btn-secondary settings-btn" onClick={() => void revealItemInDir(folder).catch(() => {})}>
            <FolderOpenIcon size={15} />
            Show in {fileBrowser}
          </button>
        )}
        <span className={folderError ? "field-note is-error" : "field-note"}>
          {folderError ?? "Pictures you add, AI results and community packs stay there between launches. Copy the folder to back them up."}
        </span>
      </div>
    </>
  );
}

function AiKeys({ onKeysChanged, toast }: { onKeysChanged: () => void; toast: Toast }) {
  const [catalogue, setCatalogue] = useState<AiCatalogue | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [providerId, setProviderId] = useState("");

  const load = useCallback(async () => {
    try {
      const c = await api.aiCatalogue();
      setCatalogue(c);
      setProviderId((p) => p || c.providers.find((x) => x.has_key)?.id || c.providers[0]?.id || "");
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);

  if (error) return <p className="field-note is-error">{error}</p>;
  if (!catalogue)
    return (
      <p className="field-note">
        <LoaderIcon /> Loading providers…
      </p>
    );
  return (
    <>
      <p className="field-note">
        FolderSkin has no server. With a key, Generate with AI sends your request straight from this computer to the provider,
        billed to your account.
      </p>
      <ProviderKeys
        catalogue={catalogue}
        providerId={providerId}
        onProvider={setProviderId}
        onChanged={() => {
          void load();
          onKeysChanged();
        }}
        toast={toast}
      />
    </>
  );
}

function Sharing() {
  const [prefs, setPrefs] = useState<SharingPrefs>(loadSharingPrefs);
  const update = (next: SharingPrefs) => {
    setPrefs(next);
    saveSharingPrefs(next);
  };
  const bad = prefs.author !== "" && !isGithubUser(prefs.author);

  return (
    <>
      <p className="field-note">
        Shared skins go on GitHub, where anyone can add them to FolderSkin. These fill in the share dialog for you.
      </p>
      <label className="field">
        <span className="field-label">Your GitHub user name</span>
        <input
          className="input"
          value={prefs.author}
          maxLength={39}
          placeholder="octocat"
          spellCheck={false}
          autoComplete="off"
          onChange={(e) => update({ ...prefs, author: e.target.value.trim() })}
        />
        {bad && <span className="field-note is-error">A GitHub user name is letters, digits and single dashes.</span>}
      </label>
      <label className="field">
        <span className="field-label">Licence for what you share</span>
        <select className="input" value={prefs.license} onChange={(e) => update({ ...prefs, license: e.target.value })}>
          {LICENSES.map((l) => (
            <option key={l.id} value={l.id}>
              {l.label}: {l.note}
            </option>
          ))}
        </select>
        <span className="field-note">FolderSkin is MIT licensed, and shared skins use Creative Commons or MIT.</span>
      </label>
      <button type="button" className="link-btn settings-link" onClick={() => void openUrl(PACKS_GUIDE_URL).catch(() => {})}>
        How sharing works
      </button>
    </>
  );
}

function About({ note }: { note: string }) {
  const link = (url: string, label: string) => (
    <button type="button" className="about-link" onClick={() => void openUrl(url).catch(() => {})}>
      {label}
    </button>
  );
  return (
    <>
      <div className="settings-about">
        <img className="settings-mark" src="/brand-mark.png" alt="" draggable={false} />
        <div>
          <p className="about-title">
            FolderSkin <span className="about-version">v{__APP_VERSION__}</span>
          </p>
          <p className="about-line">Free and open source · MIT</p>
        </div>
      </div>
      {note && <p className="field-note">{note}</p>}
      <div className="about-links">
        {link(REPO_URL, "Source code")}
        {link(`${REPO_URL}/issues`, "Report a problem")}
        {link(`${REPO_URL}/releases`, "Releases")}
      </div>
      <p className="field-note">
        Icons from Lucide (ISC) and lucide-animated (MIT). Community skins belong to the people who shared them, under the licence
        each one names.
      </p>
    </>
  );
}
