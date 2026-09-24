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
  MAX_PROFILE_NAME,
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
import { GithubMark } from "./icons/githubMark";
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
import { Brand, branded } from "./Brand";

export type SettingsTab = "general" | "ai" | "sharing" | "about";

/**
 * The pages down the side, and the words each answers to in the search above them: its section
 * titles and the words of its settings. A section's title, and a row's own words (its label and
 * `find`), light it up on its page.
 */
const PAGES: { id: SettingsTab; label: string; Icon: typeof SunIcon; find: string }[] = [
  {
    id: "general",
    label: "General",
    Icon: SlidersHorizontalIcon,
    find: "appearance theme dark light system mode accent colour color motion animation reduce reduced window sidebar rail icons your skins folder storage backup",
  },
  {
    id: "ai",
    label: "AI Provider",
    Icon: SparklesIcon,
    find: "ai where pictures are made key keys provider providers api openai xai grok recraft google gemini black forest labs flux stability ideogram fal replicate local model your machine set up generate free remove delete",
  },
  {
    id: "sharing",
    label: "Sharing",
    Icon: EarthIcon,
    find: "sharing share github connect account author credit credited licence license profile profiles cc0 cc by mit community pack",
  },
  {
    id: "about",
    label: "About",
    Icon: InfoIcon,
    find: "about folderskin what skins packs photos design ai local model free open source version newer versions update updates release releases links source code issues star",
  },
];

type Toast = (text: string, opts?: { tone?: ToastTone }) => void;

/** What's typed in the search, lowercased, so rows on the page can light up when they match. */
const Query = createContext("");

/** Every word typed is somewhere in `text`, in any order. */
const hasWords = (query: string, text: string) => {
  const t = text.toLowerCase();
  return query.split(/\s+/).every((w) => t.includes(w));
};

/** Whether a setting lights up: from two letters on, as one is in nearly everything. */
const matches = (query: string, text: string) => query.length >= 2 && hasWords(query, text);

/**
 * Every setting in one dialog, its pages down the left and the chosen one on the right: how the
 * app looks and where your skins are, the AI providers, how what you share is credited and
 * licensed, and about the app. The sidebar's dark mode switch and rail button, and the studio's
 * key dialog, change the same settings.
 */
export function Settings({
  tab: first = "general",
  folderPicture,
  themePref,
  onThemePref,
  rail,
  onRail,
  fileBrowser,
  savedCount,
  onKeysChanged,
  onClose,
  toast,
  updates,
  onCheckUpdates,
  onShowUpdate,
}: {
  tab?: SettingsTab;
  /** The plain folder as FolderSkin draws it, for the row about where skins are kept. */
  folderPicture?: string | null;
  themePref: ThemePref;
  onThemePref: (pref: ThemePref) => void;
  /** The sidebar is folded to its rail of icons. */
  rail: boolean;
  onRail: (rail: boolean) => void;
  fileBrowser: string;
  /** Skins saved on this computer: your own and community ones. */
  savedCount: number;
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
  /** The page open has something leaving it would lose: a profile being changed, an Undo, a connection to GitHub. */
  const [busy, setBusy] = useState(false);
  const query = search.trim().toLowerCase();
  const shown = useMemo(() => PAGES.filter((p) => !query || hasWords(query, `${p.label} ${p.find}`)), [query]);
  // Searching moves to the first page that has it, unless the one open has it too, or has
  // something open that moving would lose.
  useEffect(() => {
    if (!busy && shown.length > 0 && !shown.some((p) => p.id === tab)) setTab(shown[0].id);
  }, [shown, tab, busy]);
  const tabs = useRef<Record<string, HTMLButtonElement | null>>({});
  const page = PAGES.find((p) => p.id === tab) ?? PAGES[0];
  const panelId = useId();
  /** The open page's tab is in the list; when it isn't, the page is named without it. */
  const listed = shown.some((p) => p.id === tab);
  const empty = shown.length === 0;

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
            {shown.map((p, i) => (
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
                tabIndex={p.id === tab || (!listed && i === 0) ? 0 : -1}
                data-modal-focus={p.id === tab ? "" : undefined}
                className={p.id === tab ? "settings-nav-btn is-active" : "settings-nav-btn"}
                onClick={() => setTab(p.id)}
                onKeyDown={onNavKey}
              >
                <p.Icon size={17} />
                <span className="settings-nav-text">{p.label}</span>
              </button>
            ))}
          </div>
        </nav>
        {/* With nothing found the page stays, out of sight, so nothing open on it is lost. */}
        <div
          className={empty ? "settings-main is-empty" : "settings-main"}
          role="tabpanel"
          id={panelId}
          aria-labelledby={listed ? `${panelId}-${page.id}` : undefined}
          aria-label={listed ? undefined : empty ? "settings" : page.label}
          key={tab}
        >
          {empty && <p className="settings-empty">No setting matches “{search.trim()}”</p>}
          <Query.Provider value={query}>
            {tab === "general" && (
              <General
                folderPicture={folderPicture}
                themePref={themePref}
                onThemePref={onThemePref}
                rail={rail}
                onRail={onRail}
                fileBrowser={fileBrowser}
                savedCount={savedCount}
              />
            )}
            {tab === "ai" && <AiPage onKeysChanged={onKeysChanged} toast={toast} />}
            {tab === "sharing" && <Sharing toast={toast} onBusy={setBusy} />}
            {tab === "about" && <About fileBrowser={fileBrowser} updates={updates} onCheckUpdates={onCheckUpdates} onShowUpdate={onShowUpdate} />}
          </Query.Provider>
        </div>
      </div>
    </Modal>
  );
}

// ---------- the pieces every page is made of ----------

/** A part of a page under its title. `find` is more words search lights the title for. */
function Section({ title, note, find = "", children }: { title: string; note?: ReactNode; find?: string; children: ReactNode }) {
  const lit = matches(useContext(Query), `${title} ${find}`);
  return (
    <section className="set-section">
      <h2 className={lit ? "set-section-title is-match" : "set-section-title"}>{title}</h2>
      {note && <p className="set-section-note">{branded(note)}</p>}
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
        <span className="set-row-label">{branded(label)}</span>
        {note && <span className="set-row-note">{branded(note)}</span>}
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
  folderPicture,
  themePref,
  onThemePref,
  rail,
  onRail,
  fileBrowser,
  savedCount,
}: {
  folderPicture?: string | null;
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
                style={{ "--swatch": `var(--swatch-${a.id})` } as CSSProperties}
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
          lead={folderPicture ? <img className="set-row-folder" src={folderPicture} alt="" draggable={false} /> : undefined}
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
      find={`key keys api provider providers model ${catalogue?.providers.map((p) => p.label).join(" ") ?? ""}`}
      note="FolderSkin has no server. With a key, Generate with AI sends your request straight from your machine to the provider, billed to your account. The Local Model makes them for free, once it's set up."
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

/** Whether the focus is nowhere, having gone with what it was on, or inside `el`. */
const focusIsIn = (el: HTMLElement | null) => {
  const at = document.activeElement;
  return !at || at === document.body || !!el?.contains(at);
};

function Sharing({ toast, onBusy }: { toast: Toast; onBusy: (busy: boolean) => void }) {
  const [profiles, setProfiles] = useState<Profiles>(loadProfiles);
  // The profiles as last changed, for what finishes later than the render that started it.
  const latest = useRef(profiles);
  const change = useCallback((next: Profiles) => {
    latest.current = next;
    setProfiles(next);
    saveProfiles(next);
  }, []);
  /** The default profile takes the GitHub name when it has none yet. */
  const creditTo = useCallback(
    (login: string) => {
      const p = defaultProfile(latest.current);
      if (!p.author) change(upsertProfile(latest.current, { ...p, author: login }));
    },
    [change],
  );

  const [account, setAccount] = useState<GithubAccount | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [listBusy, setListBusy] = useState(false);
  useEffect(() => onBusy(connecting || listBusy), [connecting, listBusy, onBusy]);
  useEffect(() => () => onBusy(false), [onBusy]);
  useEffect(() => {
    let live = true;
    void api
      .githubAccount()
      .then((who) => {
        if (!live || !who) return;
        setAccount(who);
        creditTo(who.login);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [creditTo]);

  // Connecting, cancelling and disconnecting each take away the button that had the focus; it
  // goes to the one in the GitHub row now.
  const connectBox = useRef<HTMLDivElement>(null);
  const githubButton = useRef<HTMLButtonElement>(null);
  const refocus = useRef(false);
  useEffect(() => {
    if (!refocus.current) return;
    refocus.current = false;
    githubButton.current?.focus();
  });

  return (
    <>
      <Section title="GitHub">
        {connecting ? (
          <div className="set-block" ref={connectBox}>
            <GithubConnect
              onConnected={(who) => {
                refocus.current = focusIsIn(connectBox.current);
                setAccount(who);
                setConnecting(false);
                creditTo(who.login);
                toast(`Connected to GitHub as ${who.login}`, { tone: "ok" });
              }}
              onCancel={() => {
                refocus.current = focusIsIn(connectBox.current);
                setConnecting(false);
              }}
            />
          </div>
        ) : account ? (
          <Row label={account.login} note={account.name || "Connected"} find="github account connected disconnect" lead={<GithubAvatar account={account} />}>
            <button
              ref={githubButton}
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => {
                void api.githubSignOut().catch(() => {});
                refocus.current = true;
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
                <GithubMark size={17} />
              </span>
            }
          >
            <button ref={githubButton} type="button" className="btn btn-primary btn-sm" onClick={() => setConnecting(true)}>
              Connect
            </button>
          </Row>
        )}
      </Section>

      <Section
        title="Licence profiles"
        note="A pack you share says who made it and how others may use it. Keep a profile for each way you share: sharing starts from the default one."
      >
        <ProfileList profiles={profiles} account={account} onChange={change} onBusy={setListBusy} />
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

function ProfileList({
  profiles,
  account,
  onChange,
  onBusy,
}: {
  profiles: Profiles;
  account: GithubAccount | null;
  onChange: (p: Profiles) => void;
  /** A profile is open for changes, or Undo is offered. */
  onBusy: (busy: boolean) => void;
}) {
  /** The profile open for changes, or a new one not yet kept. */
  const [editing, setEditing] = useState<{ profile: LicenceProfile; isNew: boolean } | null>(null);
  /** The profiles as they were before one was deleted, and that one, while Undo is offered. */
  const [deleted, setDeleted] = useState<{ before: Profiles; id: string; name: string } | null>(null);
  useEffect(() => onBusy(editing !== null || deleted !== null), [editing, deleted, onBusy]);

  // Saving, cancelling, deleting and undoing each take away the button that had the focus. The
  // buttons it can go to instead are kept here by name ("add", "undo", "change <id>"), and
  // `focusNext` lists where it goes after the next render, the first that's there.
  const buttons = useRef(new Map<string, HTMLElement>());
  const hold = (key: string) => (el: HTMLElement | null) => {
    if (el) buttons.current.set(key, el);
    else buttons.current.delete(key);
  };
  const focusNext = useRef<string[] | null>(null);
  useEffect(() => {
    const want = focusNext.current;
    if (!want) return;
    focusNext.current = null;
    const found = want.map((key) => buttons.current.get(key)).find(Boolean);
    (found ?? [...buttons.current].find(([key]) => key.startsWith("change "))?.[1])?.focus();
  });

  useEffect(() => {
    if (!deleted) return;
    const t = window.setTimeout(() => {
      // Undo goes; if it had the focus, the focus goes on to adding a profile.
      if (document.activeElement === buttons.current.get("undo")) focusNext.current = ["add"];
      setDeleted(null);
    }, UNDO_MS);
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
  const close = (id: string, isNew: boolean) => {
    setEditing(null);
    focusNext.current = isNew ? ["add", `change ${id}`] : [`change ${id}`];
  };
  const save = (saved: LicenceProfile, isNew: boolean) => {
    set(upsertProfile(profiles, saved));
    close(saved.id, isNew);
  };

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
              onCancel={() => close(p.id, false)}
              onSave={(saved) => save(saved, false)}
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
                  <button
                    type="button"
                    className="icon-btn"
                    aria-label={`make ${p.name} the default`}
                    data-tip="Make default"
                    onClick={() => {
                      set(makeDefault(profiles, p.id));
                      focusNext.current = [`change ${p.id}`];
                    }}
                  >
                    <StarIcon size={15} />
                  </button>
                )}
                <button
                  ref={hold(`change ${p.id}`)}
                  type="button"
                  className="icon-btn"
                  aria-label={`change ${p.name}`}
                  data-tip="Change"
                  onClick={() => setEditing({ profile: p, isNew: false })}
                >
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
                      setDeleted({ before: profiles, id: p.id, name: p.name });
                      focusNext.current = ["undo"];
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
            onCancel={() => close(editing.profile.id, true)}
            onSave={(saved) => save(saved, true)}
          />
        )}
      </div>
      {/* Always there, so what comes into it is read out as it comes. */}
      <div role="status">
        {deleted && (
          <p className="set-undo">
            Deleted {deleted.name}.
            <button
              ref={hold("undo")}
              type="button"
              className="link-btn"
              onClick={() => {
                onChange(deleted.before);
                setDeleted(null);
                focusNext.current = [`change ${deleted.id}`];
              }}
            >
              Undo
            </button>
          </p>
        )}
      </div>
      {!editing && profiles.list.length < MAX_PROFILES && (
        <button ref={hold("add")} type="button" className="btn btn-secondary btn-sm set-add" onClick={add}>
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
  const form = useRef<HTMLFormElement>(null);
  const nameRef = useRef<HTMLInputElement>(null);
  const authorRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    nameRef.current?.focus({ preventScroll: true });
    // All of it in view, its buttons too, not only the field that has the focus.
    form.current?.scrollIntoView({ block: "nearest" });
  }, []);

  const save = () => {
    setTried(true);
    if (!problem) onSave({ ...profile, name, author: author.trim(), license });
    else if (problem.field === "name") nameRef.current?.focus();
    else if (problem.field === "author") authorRef.current?.focus();
  };

  /** What's wrong with `field`, under it, and the attributes that tie its input to it. */
  const wrong = (field: "name" | "author" | "list") => (tried && problem?.field === field ? problem.text : null);
  const note = (field: "name" | "author" | "list") =>
    wrong(field) && (
      <p id={`${id}-problem`} className="field-note is-error" role="alert">
        {wrong(field)}
      </p>
    );
  const marks = (field: "name" | "author") => (wrong(field) ? { "aria-invalid": true, "aria-describedby": `${id}-problem` } : {});

  return (
    <form
      ref={form}
      className="set-profile-form"
      aria-label={profile.name ? `change ${profile.name}` : "new profile"}
      onSubmit={(e) => {
        e.preventDefault();
        save();
      }}
    >
      <div className="field">
        <label className="field-label" htmlFor={`${id}-name`}>
          Name
        </label>
        <input
          ref={nameRef}
          id={`${id}-name`}
          className="input"
          value={name}
          maxLength={MAX_PROFILE_NAME}
          placeholder="Personal, or For work"
          onChange={(e) => setName(e.target.value)}
          {...marks("name")}
        />
        {note("name")}
      </div>
      <div className="field">
        <label className="field-label" htmlFor={`${id}-author`}>
          Credited to
        </label>
        <div className="set-author">
          <input
            ref={authorRef}
            id={`${id}-author`}
            className="input"
            value={author}
            maxLength={39}
            spellCheck={false}
            autoCapitalize="off"
            placeholder="A GitHub user name"
            onChange={(e) => setAuthor(e.target.value)}
            {...marks("author")}
          />
          {account && author.trim() !== account.login && (
            <button type="button" className="btn btn-ghost btn-sm" data-tip={account.login} onClick={() => setAuthor(account.login)}>
              Use GitHub name
            </button>
          )}
        </div>
        {note("author")}
      </div>
      <div className="field">
        <span className="field-label">Licence</span>
        <Select label="licence" className="is-field" value={license} onChange={setLicense} options={LICENCE_OPTIONS} />
      </div>
      {note("list")}
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
  fileBrowser,
  updates,
  onCheckUpdates,
  onShowUpdate,
}: {
  fileBrowser: string;
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
      <div className="set-block about-pitch">
        <p className="about-tagline">Give any folder a skin.</p>
        <p className="about-intro">
          Your best memories wear the same plain folder as your old paperwork. <Brand /> gives every folder a look that fits
          what&apos;s inside: a golden-hour film still for summer photos, a vintage travel poster for a trip, pop art for a video
          project, soft pastels for a birthday.
        </p>
        <ul className="about-points">
          <li>Skins from free community packs, from your own photos, or designed by you.</li>
          <li>Describe a style and AI paints it, with your own key or free with the Local Model.</li>
          <li>Skins show up right in {fileBrowser}, and any folder gets its own icon back in one click.</li>
          <li>Free and open source. No account, no tracking.</li>
        </ul>
      </div>
      <Section title="Updates">
        <Row label="Newer versions" note="They come from FolderSkin's releases on GitHub, and install when you say so." find="update updates release version">
          <UpdateButton status={updates} onCheck={onCheckUpdates} onShow={onShowUpdate} />
        </Row>
      </Section>
      <Section title="Links">
        <div className="set-block about-links">
          {link(REPO_URL, "Source code", <GithubMark size={15} />)}
          {link(`${REPO_URL}/issues`, "Report issues", <BadgeAlertIcon size={15} />)}
          {link(`${REPO_URL}/releases`, "Releases", <DownloadIcon size={15} />)}
          {link(REPO_URL, "Star project", <StarIcon size={15} className="about-star" />)}
        </div>
      </Section>
    </>
  );
}
