import { createContext, useCallback, useContext, useEffect, useId, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode } from "react";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type AiCatalogue, type GithubAccount } from "../lib/tauri";
import { prettyPath } from "../lib/files";
import { keys } from "../lib/platform";
import { LICENSES, PACKS_GUIDE_URL, REPO_URL } from "../lib/packs";
import {
  defaultProfile,
  loadProfiles,
  makeDefault,
  MAX_PROFILES,
  newProfileId,
  profileProblem,
  removeProfile,
  saveProfiles,
  upsertProfile,
  type LicenceProfile,
  type LicenseId,
  type Profiles,
} from "../lib/profiles";
import { ACCENTS, setPrefs, usePrefs, type Motion } from "../state/prefs";
import type { ThemePref } from "../state/theme";
import type { ToastTone } from "../hooks/useToasts";
import type { UpdateStatus } from "../hooks/useUpdates";
import { GithubAvatar } from "./GithubAvatar";
import { GithubConnect } from "./GithubConnect";
import { Modal } from "./Modal";
import { ProviderKeys } from "./ProviderKeys";
import { Select } from "./Select";
import { UpdateButton } from "./UpdateDialog";
import { BadgeAlertIcon } from "./icons/badge-alert";
import { PlusIcon, TrashIcon } from "./icons/composer";
import { DownloadIcon } from "./icons/download";
import { EarthIcon } from "./icons/earth";
import { ExternalLinkIcon } from "./icons/external-link";
import { GithubIcon } from "./icons/github";
import { InfoIcon } from "./icons/info";
import { LoaderIcon } from "./icons/loader";
import { MonitorCheckIcon } from "./icons/monitor-check";
import { MoonIcon } from "./icons/moon";
import { PencilIcon } from "./icons/pencil";
import { SearchIcon } from "./icons/search";
import { SlidersHorizontalIcon } from "./icons/sliders-horizontal";
import { SparklesIcon } from "./icons/sparkles";
import { StarIcon } from "./icons/star";
import { SunIcon } from "./icons/sun";

export type SettingsTab = "general" | "ai" | "sharing" | "about";

/**
 * The pages down the side, and the words each answers to in the search above them. A row's own
 * words (its label and `find`) light it up on its page.
 */
const PAGES: { id: SettingsTab; label: string; Icon: typeof SunIcon; find: string }[] = [
  {
    id: "general",
    label: "General",
    Icon: SlidersHorizontalIcon,
    find: "appearance theme dark light system mode accent colour color motion animation reduce sidebar rail icons window skins folder storage backup",
  },
  {
    id: "ai",
    label: "AI",
    Icon: SparklesIcon,
    find: "ai key keys provider providers api openai gemini google fal replicate flux ideogram local this computer model generate pictures",
  },
  {
    id: "sharing",
    label: "Sharing",
    Icon: EarthIcon,
    find: "sharing share github connect account author credit licence license profile profiles cc0 cc by mit community pack",
  },
  { id: "about", label: "About", Icon: InfoIcon, find: "about version update updates release releases source code issues star" },
];

type Toast = (text: string, opts?: { tone?: ToastTone }) => void;

/** What's typed in the search, lowercased, so rows on the page can light up when they match. */
const Query = createContext("");

const matches = (query: string, text: string) => query !== "" && text.toLowerCase().includes(query);

/**
 * Every setting in one dialog, its pages down the left and the chosen one on the right: how the
 * app looks and where your skins are, the AI providers, how what you share is credited and
 * licensed, and about the app. The sidebar's dark mode switch and rail button, and the studio's
 * key dialog, change the same settings.
 */
export function Settings({
  tab: first = "general",
  themePref,
  onThemePref,
  rail,
  onRail,
  fileBrowser,
  savedCount,
  note,
  onKeysChanged,
  onClose,
  toast,
  updates,
  onCheckUpdates,
  onShowUpdate,
}: {
  tab?: SettingsTab;
  themePref: ThemePref;
  onThemePref: (pref: ThemePref) => void;
  /** The sidebar is folded to its rail of icons. */
  rail: boolean;
  onRail: (rail: boolean) => void;
  fileBrowser: string;
  /** Skins saved on this computer: your own and community ones. */
  savedCount: number;
  /** How this OS keeps a folder's icon. */
  note: string;
  /** A key was saved or removed, so the studio reloads its providers. */
  onKeysChanged: () => void;
  onClose: () => void;
  toast: Toast;
  updates: UpdateStatus;
  onCheckUpdates: () => void;
  onShowUpdate: () => void;
}) {
  const [tab, setTab] = useState<SettingsTab>(first);
  const [search, setSearch] = useState("");
  const query = search.trim().toLowerCase();
  const shown = useMemo(() => PAGES.filter((p) => !query || `${p.label} ${p.find}`.toLowerCase().includes(query)), [query]);
  // Searching moves to the first page that has it, unless the one open has it too.
  useEffect(() => {
    if (shown.length > 0 && !shown.some((p) => p.id === tab)) setTab(shown[0].id);
  }, [shown, tab]);
  const tabs = useRef<Record<string, HTMLButtonElement | null>>({});
  const page = PAGES.find((p) => p.id === tab) ?? PAGES[0];
  const panelId = useId();

  /** Up and down the pages, as a list of tabs goes, choosing as it moves. */
  const onNavKey = (e: KeyboardEvent<HTMLButtonElement>) => {
    const at = shown.findIndex((p) => p.id === tab);
    let next = -1;
    if (e.key === "ArrowDown") next = (at + 1) % shown.length;
    else if (e.key === "ArrowUp") next = (at - 1 + shown.length) % shown.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = shown.length - 1;
    if (next === -1) return;
    e.preventDefault();
    setTab(shown[next].id);
    tabs.current[shown[next].id]?.focus();
  };

  return (
    <Modal bare title="Settings" className="modal-settings" onClose={onClose}>
      <div className="settings">
        <nav className="settings-nav" aria-label="settings">
          <label className="settings-search">
            <SearchIcon size={15} />
            <input
              type="search"
              value={search}
              placeholder="Search"
              aria-label="search settings"
              spellCheck={false}
              onChange={(e) => setSearch(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && shown[0]) tabs.current[shown[0].id]?.focus();
              }}
            />
          </label>
          <p className="settings-nav-label">Settings</p>
          <div className="settings-nav-list" role="tablist" aria-orientation="vertical" aria-label="settings pages">
            {shown.map((p) => (
              <button
                key={p.id}
                ref={(el) => {
                  tabs.current[p.id] = el;
                }}
                type="button"
                role="tab"
                id={`${panelId}-${p.id}`}
                aria-selected={p.id === tab}
                aria-controls={panelId}
                tabIndex={p.id === tab ? 0 : -1}
                data-modal-focus={p.id === tab ? "" : undefined}
                className={p.id === tab ? "settings-nav-btn is-active" : "settings-nav-btn"}
                onClick={() => setTab(p.id)}
                onKeyDown={onNavKey}
              >
                <p.Icon size={17} />
                <span className="settings-nav-text">{p.label}</span>
              </button>
            ))}
            {shown.length === 0 && <p className="settings-nav-empty">No setting matches “{search.trim()}”</p>}
          </div>
        </nav>
        <div className="settings-main" role="tabpanel" id={panelId} aria-labelledby={`${panelId}-${page.id}`} key={tab}>
          <Query.Provider value={query}>
            {tab === "general" && <General themePref={themePref} onThemePref={onThemePref} rail={rail} onRail={onRail} fileBrowser={fileBrowser} savedCount={savedCount} />}
            {tab === "ai" && <AiPage onKeysChanged={onKeysChanged} toast={toast} />}
            {tab === "sharing" && <Sharing toast={toast} />}
            {tab === "about" && <About note={note} updates={updates} onCheckUpdates={onCheckUpdates} onShowUpdate={onShowUpdate} />}
          </Query.Provider>
        </div>
      </div>
    </Modal>
  );
}

// ---------- the pieces every page is made of ----------

function Section({ title, note, children }: { title: string; note?: ReactNode; children: ReactNode }) {
  const id = useId();
  return (
    <section className="set-section" aria-labelledby={id}>
      <h3 className="set-section-title" id={id}>
        {title}
      </h3>
      {note && <p className="set-section-note">{note}</p>}
      <div className="set-rows">{children}</div>
    </section>
  );
}

/** A setting: what it is on the left, how it's set on the right. `find` is more words search knows it by. */
function Row({ label, note, find = "", lead, children }: { label: ReactNode; note?: ReactNode; find?: string; lead?: ReactNode; children?: ReactNode }) {
  const query = useContext(Query);
  const lit = matches(query, `${typeof label === "string" ? label : ""} ${find}`);
  return (
    <div className={lit ? "set-row is-match" : "set-row"}>
      {lead}
      <div className="set-row-text">
        <span className="set-row-label">{label}</span>
        {note && <span className="set-row-note">{note}</span>}
      </div>
      {children !== undefined && <div className="set-row-control">{children}</div>}
    </div>
  );
}

type SegOption<T> = { value: T; label?: string; Icon?: typeof SunIcon; tip?: string };

/**
 * One of a few, side by side, the chosen one raised. With icons alone each names itself in a
 * tooltip. Arrow keys move the choice, as radio buttons do.
 */
function Seg<T extends string | boolean>({ label, value, options, onChange }: { label: string; value: T; options: SegOption<T>[]; onChange: (v: T) => void }) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const onKey = (e: KeyboardEvent<HTMLButtonElement>, i: number) => {
    const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = (i + step + options.length) % options.length;
    onChange(options[next].value);
    refs.current[next]?.focus();
  };
  const iconOnly = options.every((o) => o.Icon && !o.label);
  return (
    <div className={iconOnly ? "set-seg is-icons" : "set-seg"} role="radiogroup" aria-label={label}>
      {options.map((o, i) => (
        <button
          key={String(o.value)}
          ref={(el) => {
            refs.current[i] = el;
          }}
          type="button"
          role="radio"
          aria-checked={o.value === value}
          aria-label={o.label ? undefined : o.tip}
          tabIndex={o.value === value ? 0 : -1}
          data-tip={o.tip}
          className={o.value === value ? "set-seg-btn is-on" : "set-seg-btn"}
          onClick={() => onChange(o.value)}
          onKeyDown={(e) => onKey(e, i)}
        >
          {o.Icon && <o.Icon size={16} />}
          {o.label}
        </button>
      ))}
    </div>
  );
}

// ---------- General ----------

const THEMES: SegOption<ThemePref>[] = [
  { value: "system", Icon: MonitorCheckIcon, tip: "Match the computer" },
  { value: "light", Icon: SunIcon, tip: "Light" },
  { value: "dark", Icon: MoonIcon, tip: "Dark" },
];

const MOTIONS: SegOption<Motion>[] = [
  { value: "system", label: "System", tip: "As much as the computer's own setting allows" },
  { value: "reduced", label: "Reduced", tip: "Things appear and change without moving" },
];

function General({
  themePref,
  onThemePref,
  rail,
  onRail,
  fileBrowser,
  savedCount,
}: {
  themePref: ThemePref;
  onThemePref: (pref: ThemePref) => void;
  rail: boolean;
  onRail: (rail: boolean) => void;
  fileBrowser: string;
  savedCount: number;
}) {
  const prefs = usePrefs();
  const [folder, setFolder] = useState<string | null>(null);
  const [folderError, setFolderError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    api
      .skinsFolder()
      .then((f) => live && setFolder(f))
      .catch((e) => live && setFolderError(errorMessage(e)));
    return () => {
      live = false;
    };
  }, []);
  const swatches = useRef<(HTMLButtonElement | null)[]>([]);
  const onSwatchKey = (e: KeyboardEvent<HTMLButtonElement>, i: number) => {
    const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = (i + step + ACCENTS.length) % ACCENTS.length;
    setPrefs({ accent: ACCENTS[next].id });
    swatches.current[next]?.focus();
  };

  return (
    <>
      <Section title="Appearance">
        <Row label="Theme" find="dark light system mode">
          <Seg label="theme" value={themePref} options={THEMES} onChange={onThemePref} />
        </Row>
        <Row label="Accent colour" note="Buttons, selections and the glow around a folder you drop." find="color accent">
          <div className="set-swatches" role="radiogroup" aria-label="accent colour">
            {ACCENTS.map((a, i) => (
              <button
                key={a.id}
                ref={(el) => {
                  swatches.current[i] = el;
                }}
                type="button"
                role="radio"
                aria-checked={prefs.accent === a.id}
                aria-label={a.label}
                tabIndex={prefs.accent === a.id ? 0 : -1}
                data-tip={a.label}
                className={["set-swatch", a.id === "mono" && "is-mono", prefs.accent === a.id && "is-on"].filter(Boolean).join(" ")}
                style={a.id === "mono" ? undefined : ({ "--swatch": a.swatch } as CSSProperties)}
                onClick={() => setPrefs({ accent: a.id })}
                onKeyDown={(e) => onSwatchKey(e, i)}
              />
            ))}
          </div>
        </Row>
        <Row label="Motion" note="Less movement in the library, the dialogs and the icons." find="animation reduce reduced">
          <Seg label="motion" value={prefs.motion} options={MOTIONS} onChange={(motion) => setPrefs({ motion })} />
        </Row>
      </Section>

      <Section title="Window">
        <Row label="Sidebar" note={`Icons only folds it to a narrow strip. ${keys("\\")} switches between them.`} find="rail collapse fold icons">
          <Seg
            label="sidebar"
            value={rail}
            options={[
              { value: false, label: "Full" },
              { value: true, label: "Icons only" },
            ]}
            onChange={onRail}
          />
        </Row>
      </Section>

      <Section title="Your skins" note={folderError ?? "Everything you add stays here between launches. Copy this folder to back it up."}>
        <Row
          label={savedCount === 1 ? "1 skin" : `${savedCount} skins`}
          find="storage folder backup skins"
          lead={
            <span className="set-row-lead" aria-hidden="true">
              <FolderLead />
            </span>
          }
          note={
            <span className="set-path" data-tip={folder ?? undefined} data-tip-overflow>
              {folder ? prettyPath(folder) : folderError ? "Kept until you quit" : " "}
            </span>
          }
        >
          {folder && (
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => void revealItemInDir(folder).catch(() => {})}>
              Show in {fileBrowser}
            </button>
          )}
        </Row>
      </Section>
    </>
  );
}

function FolderLead() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />
    </svg>
  );
}

// ---------- AI ----------

function AiPage({ onKeysChanged, toast }: { onKeysChanged: () => void; toast: Toast }) {
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

  return (
    <Section
      title="Where pictures are made"
      note="FolderSkin has no server. With a key, Generate with AI sends your request straight from this computer to the provider, billed to your account. This computer makes them for free, once it's set up."
    >
      <div className="set-block">
        {error ? (
          <p className="field-note is-error">{error}</p>
        ) : !catalogue ? (
          <p className="field-note">
            <LoaderIcon /> Loading providers
          </p>
        ) : (
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
        )}
      </div>
    </Section>
  );
}

// ---------- Sharing ----------

function Sharing({ toast }: { toast: Toast }) {
  const [profiles, setProfiles] = useState<Profiles>(loadProfiles);
  const change = (next: Profiles) => {
    setProfiles(next);
    saveProfiles(next);
  };

  const [account, setAccount] = useState<GithubAccount | null>(null);
  const [connecting, setConnecting] = useState(false);
  useEffect(() => {
    let live = true;
    void api
      .githubAccount()
      .then((who) => {
        if (live && who) setAccount(who);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, []);

  if (connecting) {
    return (
      <Section title="Connect to GitHub">
        <div className="set-block">
          <GithubConnect
            onConnected={(who) => {
              setAccount(who);
              setConnecting(false);
              // The default profile takes the GitHub name when it has none yet.
              const p = defaultProfile(profiles);
              if (!p.author) change(upsertProfile(profiles, { ...p, author: who.login }));
              toast(`Connected to GitHub as ${who.login}`, { tone: "ok" });
            }}
            onCancel={() => setConnecting(false)}
          />
        </div>
      </Section>
    );
  }

  return (
    <>
      <Section title="GitHub">
        {account ? (
          <Row label={account.login} note={account.name || "Connected"} find="github account connected disconnect" lead={<GithubAvatar account={account} />}>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => {
                void api.githubSignOut().catch(() => {});
                setAccount(null);
              }}
            >
              Disconnect
            </button>
          </Row>
        ) : (
          <Row
            label="Connect to GitHub"
            note="Share packs from your own GitHub account, credited to you."
            find="github account connect sign in"
            lead={
              <span className="set-row-lead" aria-hidden="true">
                <GithubIcon size={17} />
              </span>
            }
          >
            <button type="button" className="btn btn-primary btn-sm" onClick={() => setConnecting(true)}>
              Connect
            </button>
          </Row>
        )}
      </Section>

      <Section
        title="Licence profiles"
        note="A pack you share says who made it and how others may use it. Keep a profile for each way you share: sharing starts from the default one."
      >
        <ProfileList profiles={profiles} account={account} onChange={change} />
      </Section>

      <button type="button" className="link-btn set-link" onClick={() => void openUrl(PACKS_GUIDE_URL).catch(() => {})}>
        How sharing works <ExternalLinkIcon size={12} />
      </button>
    </>
  );
}

/** How long a deleted profile can be brought back. */
const UNDO_MS = 8000;

const LICENCE_OPTIONS = LICENSES.map((l) => ({ value: l.id as LicenseId, label: `${l.label}: ${l.note}` }));
const licenceName = (id: LicenseId) => LICENSES.find((l) => l.id === id)?.label ?? id;

function ProfileList({ profiles, account, onChange }: { profiles: Profiles; account: GithubAccount | null; onChange: (p: Profiles) => void }) {
  /** The profile open for changes, or a new one not yet kept. */
  const [editing, setEditing] = useState<{ profile: LicenceProfile; isNew: boolean } | null>(null);
  /** The profiles as they were before one was deleted, and its name, while Undo is offered. */
  const [deleted, setDeleted] = useState<{ before: Profiles; name: string } | null>(null);
  useEffect(() => {
    if (!deleted) return;
    const t = window.setTimeout(() => setDeleted(null), UNDO_MS);
    return () => window.clearTimeout(t);
  }, [deleted]);
  // Anything else that changes the list takes the Undo away: it would undo that too.
  const set = (next: Profiles) => {
    setDeleted(null);
    onChange(next);
  };
  const query = useContext(Query);

  const add = () =>
    setEditing({
      profile: { id: newProfileId(), name: "", author: defaultProfile(profiles).author || account?.login || "", license: LICENSES[0].id },
      isNew: true,
    });

  return (
    <>
      <div className="set-profiles">
        {profiles.list.map((p) =>
          editing?.profile.id === p.id ? (
            <ProfileForm
              key={p.id}
              profile={editing.profile}
              others={profiles.list.filter((o) => o.id !== p.id)}
              account={account}
              onCancel={() => setEditing(null)}
              onSave={(saved) => {
                set(upsertProfile(profiles, saved));
                setEditing(null);
              }}
            />
          ) : (
            <div key={p.id} className={matches(query, `${p.name} ${p.author} ${licenceName(p.license)}`) ? "set-profile is-match" : "set-profile"}>
              <span className="set-profile-text">
                <span className="set-profile-name">
                  {p.name}
                  {p.id === profiles.defaultId && <span className="set-chip">Default</span>}
                </span>
                <span className="set-profile-meta">
                  {p.author ? `Credited to ${p.author}` : "No one to credit yet"} · {licenceName(p.license)}
                </span>
              </span>
              <span className="set-profile-actions">
                {p.id !== profiles.defaultId && (
                  <button type="button" className="icon-btn" aria-label={`make ${p.name} the default`} data-tip="Make default" onClick={() => set(makeDefault(profiles, p.id))}>
                    <StarIcon size={15} />
                  </button>
                )}
                <button type="button" className="icon-btn" aria-label={`change ${p.name}`} data-tip="Change" onClick={() => setEditing({ profile: p, isNew: false })}>
                  <PencilIcon size={15} />
                </button>
                {profiles.list.length > 1 && (
                  <button
                    type="button"
                    className="icon-btn is-danger"
                    aria-label={`delete ${p.name}`}
                    data-tip="Delete"
                    onClick={() => {
                      onChange(removeProfile(profiles, p.id));
                      setDeleted({ before: profiles, name: p.name });
                    }}
                  >
                    <TrashIcon size={15} />
                  </button>
                )}
              </span>
            </div>
          ),
        )}
        {editing?.isNew && (
          <ProfileForm
            profile={editing.profile}
            others={profiles.list}
            account={account}
            onCancel={() => setEditing(null)}
            onSave={(saved) => {
              set(upsertProfile(profiles, saved));
              setEditing(null);
            }}
          />
        )}
      </div>
      {deleted && (
        <p className="set-undo" role="status">
          Deleted {deleted.name}.
          <button
            type="button"
            className="link-btn"
            onClick={() => {
              onChange(deleted.before);
              setDeleted(null);
            }}
          >
            Undo
          </button>
        </p>
      )}
      {!editing && profiles.list.length < MAX_PROFILES && (
        <button type="button" className="btn btn-secondary btn-sm set-add" onClick={add}>
          <PlusIcon size={14} />
          Add a profile
        </button>
      )}
    </>
  );
}

function ProfileForm({
  profile,
  others,
  account,
  onSave,
  onCancel,
}: {
  profile: LicenceProfile;
  others: LicenceProfile[];
  account: GithubAccount | null;
  onSave: (p: LicenceProfile) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(profile.name);
  const [author, setAuthor] = useState(profile.author);
  const [license, setLicense] = useState<LicenseId>(profile.license);
  const [tried, setTried] = useState(false);
  const problem = profileProblem({ name, author: author.trim() }, others);
  const id = useId();
  const nameRef = useRef<HTMLInputElement>(null);
  useEffect(() => nameRef.current?.focus(), []);

  const save = () => {
    setTried(true);
    if (problem) return;
    onSave({ ...profile, name, author: author.trim(), license });
  };

  return (
    <form
      className="set-profile-form"
      aria-label={profile.name ? `change ${profile.name}` : "new profile"}
      onSubmit={(e) => {
        e.preventDefault();
        save();
      }}
    >
      <label className="field" htmlFor={`${id}-name`}>
        <span className="field-label">Name</span>
        <input ref={nameRef} id={`${id}-name`} className="input" value={name} maxLength={60} placeholder="Personal, For work…" onChange={(e) => setName(e.target.value)} />
      </label>
      <div className="field">
        <label className="field-label" htmlFor={`${id}-author`}>
          Credited to
        </label>
        <div className="set-author">
          <input
            id={`${id}-author`}
            className="input"
            value={author}
            maxLength={39}
            spellCheck={false}
            autoCapitalize="off"
            placeholder="A GitHub user name"
            onChange={(e) => setAuthor(e.target.value)}
          />
          {account && author.trim() !== account.login && (
            <button type="button" className="btn btn-ghost btn-sm" onClick={() => setAuthor(account.login)}>
              Use {account.login}
            </button>
          )}
        </div>
      </div>
      <div className="field">
        <span className="field-label">Licence</span>
        <Select label="licence" className="is-field" value={license} onChange={setLicense} options={LICENCE_OPTIONS} />
      </div>
      {tried && problem && (
        <p className="field-note is-error" role="alert">
          {problem}
        </p>
      )}
      <div className="set-form-actions">
        <button type="button" className="btn btn-ghost btn-sm" onClick={onCancel}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary btn-sm">
          {profile.name ? "Save" : "Add profile"}
        </button>
      </div>
    </form>
  );
}

// ---------- About ----------

function About({
  note,
  updates,
  onCheckUpdates,
  onShowUpdate,
}: {
  note: string;
  updates: UpdateStatus;
  onCheckUpdates: () => void;
  onShowUpdate: () => void;
}) {
  const link = (url: string, label: string, icon: ReactNode) => (
    <button type="button" className="about-link" onClick={() => void openUrl(url).catch(() => {})}>
      {icon}
      {label}
    </button>
  );
  return (
    <>
      <div className="set-about">
        <img className="set-about-mark" src="/brand-mark.png" alt="" draggable={false} />
        <div>
          <p className="about-title">
            Folder<span className="brand-accent">Skin</span> <span className="about-version">v{__APP_VERSION__}</span>
          </p>
          <p className="about-line">Free and open source · MIT</p>
        </div>
      </div>
      <Section title="Updates">
        <Row label="Newer versions" note="They come from FolderSkin's releases on GitHub, and install when you say so." find="update updates release version">
          <UpdateButton status={updates} onCheck={onCheckUpdates} onShow={onShowUpdate} />
        </Row>
      </Section>
      {note && (
        <Section title="On this computer">
          <Row label="Folder icons" note={note} find="icon desktop.ini finder explorer" />
        </Section>
      )}
      <Section title="Links">
        <div className="set-block about-links">
          {link(REPO_URL, "Source code", <GithubIcon size={15} />)}
          {link(`${REPO_URL}/issues`, "Report issues", <BadgeAlertIcon size={15} />)}
          {link(`${REPO_URL}/releases`, "Releases", <DownloadIcon size={15} />)}
          {link(REPO_URL, "Star project", <StarIcon size={15} className="about-star" />)}
        </div>
      </Section>
    </>
  );
}
