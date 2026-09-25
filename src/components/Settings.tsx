import { createContext, useCallback, useContext, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode } from "react";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type AiCatalogue } from "../lib/tauri";
import { prettyPath } from "../lib/files";
import { keys, osOf, type Os } from "../lib/platform";
import { LICENSES, REPO_URL } from "../lib/packs";
import { licenceOption } from "../lib/licences";
import { docsUrl, t as tNow, useT } from "../i18n";
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
import { branded } from "./Brand";

export type SettingsTab = "general" | "ai" | "sharing" | "about";

/**
 * The pages down the side, and the words each answers to in the search above them: its section
 * titles and the words of its settings (`settings.pages.<id>` and `settings.find.<id>`). A
 * section's title, and a row's own words (its label and `find`), light it up on its page.
 */
const PAGES: { id: SettingsTab; Icon: typeof SunIcon }[] = [
  { id: "general", Icon: SlidersHorizontalIcon },
  { id: "ai", Icon: SparklesIcon },
  { id: "sharing", Icon: EarthIcon },
  { id: "about", Icon: InfoIcon },
];

const pageLabel = (id: SettingsTab) => tNow(`settings.pages.${id}`);

/** A page's name and the words it answers to in search, in the language on show. */
const pageFind = (id: SettingsTab) => `${pageLabel(id)} ${tNow(`settings.find.${id}`)}`;

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
  os,
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
  /** The system, whose file browser the page names. */
  os: string;
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
  const t = useT();
  const [tab, setTab] = useState<SettingsTab>(first);
  const [search, setSearch] = useState("");
  /** The page open has something leaving it would lose: a profile being changed, or an Undo. */
  const [busy, setBusy] = useState(false);
  const query = search.trim().toLowerCase();
  // The words are the language's, so the pages are found again when it changes.
  const shown = useMemo(() => PAGES.filter((p) => !query || hasWords(query, pageFind(p.id))), [query, t]);
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
    <Modal bare title={t("settings.title")} className="modal-settings" onClose={onClose}>
      <div className="settings">
        <nav className="settings-nav" aria-label={t("settings.navLabel")}>
          <label className="settings-search">
            <SearchIcon size={15} />
            <input
              type="search"
              value={search}
              placeholder={t("settings.search")}
              aria-label={t("settings.searchLabel")}
              spellCheck={false}
              onChange={(e) => setSearch(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && shown[0]) tabs.current[shown[0].id]?.focus();
              }}
            />
          </label>
          <p className="settings-nav-label">{t("settings.title")}</p>
          <div className="settings-nav-list" role="tablist" aria-orientation="vertical" aria-label={t("settings.pagesLabel")}>
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
                <span className="settings-nav-text">{pageLabel(p.id)}</span>
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
          aria-label={listed ? undefined : empty ? t("settings.navLabel") : pageLabel(page.id)}
          key={tab}
        >
          {empty && <p className="settings-empty">{t("settings.noMatch", { query: search.trim() })}</p>}
          <Query.Provider value={query}>
            {tab === "general" && (
              <General
                folderPicture={folderPicture}
                themePref={themePref}
                onThemePref={onThemePref}
                rail={rail}
                onRail={onRail}
                os={osOf(os)}
                savedCount={savedCount}
              />
            )}
            {tab === "ai" && <AiPage onKeysChanged={onKeysChanged} toast={toast} />}
            {tab === "sharing" && <Sharing onBusy={setBusy} />}
            {tab === "about" && <About os={osOf(os)} updates={updates} onCheckUpdates={onCheckUpdates} onShowUpdate={onShowUpdate} />}
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

const THEMES = (): SegOption<ThemePref>[] => [
  { value: "system", Icon: MonitorCheckIcon, tip: tNow("settings.general.themes.system") },
  { value: "light", Icon: SunIcon, tip: tNow("settings.general.themes.light") },
  { value: "dark", Icon: MoonIcon, tip: tNow("settings.general.themes.dark") },
];

const MOTIONS = (): SegOption<Motion>[] => [
  { value: "system", label: tNow("settings.general.motions.system"), tip: tNow("settings.general.motions.systemTip") },
  { value: "reduced", label: tNow("settings.general.motions.reduced"), tip: tNow("settings.general.motions.reducedTip") },
];

function General({
  folderPicture,
  themePref,
  onThemePref,
  rail,
  onRail,
  os,
  savedCount,
}: {
  folderPicture?: string | null;
  themePref: ThemePref;
  onThemePref: (pref: ThemePref) => void;
  rail: boolean;
  onRail: (rail: boolean) => void;
  os: Os;
  savedCount: number;
}) {
  const t = useT();
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
      <Section title={t("settings.general.appearance")}>
        <Row label={t("settings.general.theme")} find={t("settings.general.themeFind")}>
          <Seg label={t("settings.general.themeLabel")} value={themePref} options={THEMES()} onChange={onThemePref} />
        </Row>
        <Row label={t("settings.general.accent")} note={t("settings.general.accentNote")} find={t("settings.general.accentFind")}>
          <div className="set-swatches" role="radiogroup" aria-label={t("settings.general.accentLabel")}>
            {ACCENTS.map((a, i) => (
              <button
                key={a.id}
                ref={(el) => {
                  swatches.current[i] = el;
                }}
                type="button"
                role="radio"
                aria-checked={prefs.accent === a.id}
                aria-label={t(`settings.general.accents.${a.id}`)}
                tabIndex={prefs.accent === a.id ? 0 : -1}
                data-tip={t(`settings.general.accents.${a.id}`)}
                className={["set-swatch", a.id === "mono" && "is-mono", prefs.accent === a.id && "is-on"].filter(Boolean).join(" ")}
                style={{ "--swatch": `var(--swatch-${a.id})` } as CSSProperties}
                onClick={() => setPrefs({ accent: a.id })}
                onKeyDown={(e) => onSwatchKey(e, i)}
              />
            ))}
          </div>
        </Row>
        <Row label={t("settings.general.motion")} note={t("settings.general.motionNote")} find={t("settings.general.motionFind")}>
          <Seg label={t("settings.general.motionLabel")} value={prefs.motion} options={MOTIONS()} onChange={(motion) => setPrefs({ motion })} />
        </Row>
      </Section>

      <Section title={t("settings.general.window")}>
        <Row label={t("settings.general.sidebar")} note={t("settings.general.sidebarNote", { keys: keys("\\") })} find={t("settings.general.sidebarFind")}>
          <Seg
            label={t("settings.general.sidebarLabel")}
            value={rail}
            options={[
              { value: false, label: t("settings.general.full") },
              { value: true, label: t("settings.general.iconsOnly") },
            ]}
            onChange={onRail}
          />
        </Row>
      </Section>

      <Section title={t("settings.general.yourSkins")} note={folderError ?? t("settings.general.yourSkinsNote")}>
        <Row
          label={t("settings.general.skins", { count: savedCount })}
          find={t("settings.general.skinsFind")}
          lead={folderPicture ? <img className="set-row-folder" src={folderPicture} alt="" draggable={false} /> : undefined}
          note={
            <span className="set-path" data-tip={folder ?? undefined} data-tip-overflow>
              {folder ? prettyPath(folder) : folderError ? t("settings.general.keptUntilQuit") : " "}
            </span>
          }
        >
          {folder && (
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => void revealItemInDir(folder).catch(() => {})}>
              {t(`common.showIn.${os}`)}
            </button>
          )}
        </Row>
      </Section>
    </>
  );
}

// ---------- AI ----------

function AiPage({ onKeysChanged, toast }: { onKeysChanged: () => void; toast: Toast }) {
  const t = useT();
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
      title={t("settings.ai.title")}
      find={`${t("settings.ai.find")} ${catalogue?.providers.map((p) => p.label).join(" ") ?? ""}`}
      note={t("settings.ai.note")}
    >
      <div className="set-block">
        {error ? (
          <p className="field-note is-error">{error}</p>
        ) : !catalogue ? (
          <p className="field-note">
            <LoaderIcon /> {t("settings.ai.loading")}
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

function Sharing({ onBusy }: { onBusy: (busy: boolean) => void }) {
  const t = useT();
  const [profiles, setProfiles] = useState<Profiles>(loadProfiles);
  const change = useCallback((next: Profiles) => {
    setProfiles(next);
    saveProfiles(next);
  }, []);

  const [listBusy, setListBusy] = useState(false);
  useEffect(() => onBusy(listBusy), [listBusy, onBusy]);
  useEffect(() => () => onBusy(false), [onBusy]);

  return (
    <>
      <Section
        title={t("settings.sharing.title")}
        note={t("settings.sharing.note")}
      >
        <ProfileList profiles={profiles} onChange={change} onBusy={setListBusy} />
      </Section>

      <button type="button" className="link-btn set-link" onClick={() => void openUrl(docsUrl("packs")).catch(() => {})}>
        {t("settings.sharing.howItWorks")} <ExternalLinkIcon size={12} />
      </button>
    </>
  );
}

/** How long a deleted profile can be brought back. */
const UNDO_MS = 8000;

const licenceOptions = () => LICENSES.map((l) => ({ value: l.id as LicenseId, label: licenceOption(l) }));
const licenceName = (id: LicenseId) => LICENSES.find((l) => l.id === id)?.label ?? id;

function ProfileList({
  profiles,
  onChange,
  onBusy,
}: {
  profiles: Profiles;
  onChange: (p: Profiles) => void;
  /** A profile is open for changes, or Undo is offered. */
  onBusy: (busy: boolean) => void;
}) {
  const t = useT();
  /** The profile open for changes, or a new one not yet kept. */
  const [editing, setEditing] = useState<{ profile: LicenceProfile; isNew: boolean } | null>(null);
  /** The profiles as they were before one was deleted, and that one, while Undo is offered. */
  const [deleted, setDeleted] = useState<{ before: Profiles; id: string; name: string } | null>(null);
  useEffect(() => onBusy(editing !== null || deleted !== null), [editing, deleted, onBusy]);

  // Saving, cancelling, deleting and undoing each take away the button that had the focus. The
  // buttons it can go to instead are kept here by name ("add", "undo", "change <id>"), and
  // `focusNext` lists where it goes after the next render, the first that's there. It goes as
  // that render is put on screen: an earlier render's effects still waiting to run, as they can
  // be on a busy machine, would take the list before what it names is there.
  const buttons = useRef(new Map<string, HTMLElement>());
  const hold = (key: string) => (el: HTMLElement | null) => {
    if (el) buttons.current.set(key, el);
    else buttons.current.delete(key);
  };
  const focusNext = useRef<string[] | null>(null);
  useLayoutEffect(() => {
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
      profile: { id: newProfileId(), name: "", author: defaultProfile(profiles).author, license: LICENSES[0].id },
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
              onCancel={() => close(p.id, false)}
              onSave={(saved) => save(saved, false)}
            />
          ) : (
            <div key={p.id} className={matches(query, `${p.name} ${p.author} ${licenceName(p.license)}`) ? "set-profile is-match" : "set-profile"}>
              <span className="set-profile-text">
                <span className="set-profile-name">
                  {p.name}
                  {p.id === profiles.defaultId && <span className="set-chip">{t("settings.profile.default")}</span>}
                </span>
                <span className="set-profile-meta">
                  {p.author ? t("settings.profile.creditedTo", { author: p.author }) : t("settings.profile.noCredit")} · {licenceName(p.license)}
                </span>
              </span>
              <span className="set-profile-actions">
                {p.id !== profiles.defaultId && (
                  <button
                    type="button"
                    className="icon-btn"
                    aria-label={t("settings.profile.makeDefaultLabel", { name: p.name })}
                    data-tip={t("settings.profile.makeDefault")}
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
                  aria-label={t("settings.profile.changeLabel", { name: p.name })}
                  data-tip={t("settings.profile.change")}
                  onClick={() => setEditing({ profile: p, isNew: false })}
                >
                  <PencilIcon size={15} />
                </button>
                {profiles.list.length > 1 && (
                  <button
                    type="button"
                    className="icon-btn is-danger"
                    aria-label={t("settings.profile.deleteLabel", { name: p.name })}
                    data-tip={t("settings.profile.delete")}
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
            onCancel={() => close(editing.profile.id, true)}
            onSave={(saved) => save(saved, true)}
          />
        )}
      </div>
      {/* Always there, so what comes into it is read out as it comes. */}
      <div role="status">
        {deleted && (
          <p className="set-undo">
            {t("settings.profile.deleted", { name: deleted.name })}
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
              {t("settings.profile.undo")}
            </button>
          </p>
        )}
      </div>
      {!editing && profiles.list.length < MAX_PROFILES && (
        <button ref={hold("add")} type="button" className="btn btn-secondary btn-sm set-add" onClick={add}>
          <PlusIcon size={14} />
          {t("settings.profile.add")}
        </button>
      )}
    </>
  );
}

function ProfileForm({
  profile,
  others,
  onSave,
  onCancel,
}: {
  profile: LicenceProfile;
  others: LicenceProfile[];
  onSave: (p: LicenceProfile) => void;
  onCancel: () => void;
}) {
  const t = useT();
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
      aria-label={profile.name ? t("settings.profile.changeLabel", { name: profile.name }) : t("settings.profile.newLabel")}
      onSubmit={(e) => {
        e.preventDefault();
        save();
      }}
    >
      <div className="field">
        <label className="field-label" htmlFor={`${id}-name`}>
          {t("settings.profile.name")}
        </label>
        <input
          ref={nameRef}
          id={`${id}-name`}
          className="input"
          value={name}
          maxLength={MAX_PROFILE_NAME}
          placeholder={t("settings.profile.namePlaceholder")}
          onChange={(e) => setName(e.target.value)}
          {...marks("name")}
        />
        {note("name")}
      </div>
      <div className="field">
        <label className="field-label" htmlFor={`${id}-author`}>
          {t("settings.profile.creditedToLabel")}
        </label>
        <input
          ref={authorRef}
          id={`${id}-author`}
          className="input"
          value={author}
          maxLength={39}
          spellCheck={false}
          autoCapitalize="off"
          placeholder={t("settings.profile.authorPlaceholder")}
          onChange={(e) => setAuthor(e.target.value)}
          {...marks("author")}
        />
        {note("author")}
      </div>
      <div className="field">
        <span className="field-label">{t("settings.profile.licence")}</span>
        <Select label={t("settings.profile.licenceLabel")} className="is-field" value={license} onChange={setLicense} options={licenceOptions()} />
      </div>
      {note("list")}
      <div className="set-form-actions">
        <button type="button" className="btn btn-ghost btn-sm" onClick={onCancel}>
          {t("common.cancel")}
        </button>
        <button type="submit" className="btn btn-primary btn-sm">
          {profile.name ? t("settings.profile.save") : t("settings.profile.addProfile")}
        </button>
      </div>
    </form>
  );
}

// ---------- About ----------

function About({
  os,
  updates,
  onCheckUpdates,
  onShowUpdate,
}: {
  os: Os;
  updates: UpdateStatus;
  onCheckUpdates: () => void;
  onShowUpdate: () => void;
}) {
  const t = useT();
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
          <p className="about-line">{t("common.about.licence")}</p>
        </div>
      </div>
      <div className="set-block about-pitch">
        <p className="about-tagline">{t("common.about.tagline")}</p>
        <p className="about-intro">{branded(t("settings.about.intro"))}</p>
        <ul className="about-points">
          <li>{t("settings.about.points.skins")}</li>
          <li>{t("settings.about.points.ai")}</li>
          <li>{t(`settings.about.points.fileBrowser.${os}`)}</li>
          <li>{t("settings.about.points.free")}</li>
        </ul>
      </div>
      <Section title={t("settings.about.updates")}>
        <Row label={t("settings.about.newer")} note={t("settings.about.newerNote")} find={t("settings.about.newerFind")}>
          <UpdateButton status={updates} onCheck={onCheckUpdates} onShow={onShowUpdate} />
        </Row>
      </Section>
      <Section title={t("settings.about.links")}>
        <div className="set-block about-links">
          {link(REPO_URL, t("settings.about.sourceCode"), <GithubMark size={15} />)}
          {link(`${REPO_URL}/issues`, t("common.about.reportIssues"), <BadgeAlertIcon size={15} />)}
          {link(`${REPO_URL}/releases`, t("settings.about.releases"), <DownloadIcon size={15} />)}
          {link(REPO_URL, t("common.about.star"), <StarIcon size={15} className="about-star" />)}
        </div>
      </Section>
    </>
  );
}
