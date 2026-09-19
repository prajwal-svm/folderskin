import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type PlatformInfo, type Skin } from "./lib/tauri";
import { isTauri } from "./lib/devMock";
import { IMAGE_EXTENSIONS } from "./lib/files";
import { browseLabel, fileBrowser } from "./lib/platform";
import { isYours, tagCounts, tagLabel } from "./lib/tags";
import { initialState, reduce } from "./state/dropzone";
import { loadFavorites, saveFavorites, toggleFavorite } from "./state/favorites";
import { applyTheme, loadThemePref, resolveTheme, saveThemePref, toggleTheme, type Theme, type ThemePref } from "./state/theme";
import { useDragDrop } from "./hooks/useDragDrop";
import { useToasts } from "./hooks/useToasts";
import { useUpdates } from "./hooks/useUpdates";
import { Sidebar, type View } from "./components/Sidebar";
import { GalleryToolbar, type TabCount } from "./components/GalleryToolbar";
import { Gallery, type Empty } from "./components/Gallery";
import { FolderStage } from "./components/FolderStage";
import { AboutMenu } from "./components/AboutMenu";
import { CommunityView } from "./components/CommunityView";
import { Studio } from "./components/Studio";
import { Confirm } from "./components/Confirm";
import { SkinMenu } from "./components/SkinMenu";
import { SharePack } from "./components/SharePack";
import { Settings, type SettingsTab } from "./components/Settings";
import { Toaster } from "./components/Toaster";
import { UpdateDialog } from "./components/UpdateDialog";
import { SearchIcon } from "./components/icons/search";
import { StarIcon } from "./components/icons/star";
import { FolderOpenIcon } from "./components/icons/folder-open";


function useTheme(): { theme: Theme; pref: ThemePref; setPref: (pref: ThemePref) => void; toggle: () => void } {
  const [pref, setPref] = useState<ThemePref>(() => loadThemePref());
  const [theme, setTheme] = useState<Theme>(() => resolveTheme(loadThemePref()));
  useEffect(() => {
    const t = resolveTheme(pref);
    setTheme(t);
    applyTheme(t);
    // The window's material follows the native appearance, so it has to match the app's choice.
    api.setWindowTheme(pref === "system" ? null : pref).catch(() => {});
    if (pref !== "system" || typeof matchMedia !== "function") return;
    const mq = matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      const next = resolveTheme("system");
      setTheme(next);
      applyTheme(next);
    };
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [pref]);
  const toggle = useCallback(() => {
    setPref((p) => {
      const next = toggleTheme(resolveTheme(p));
      saveThemePref(next);
      return next;
    });
  }, []);
  const choose = useCallback((next: ThemePref) => {
    saveThemePref(next);
    setPref(next);
  }, []);
  return { theme, pref, setPref: choose, toggle };
}

/** Newest first. A pack keeps its own order: the app gives its first skin the newest time. */
function newestFirst(list: Skin[]): Skin[] {
  return [...list].sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));
}

export default function App() {
  const [state, dispatch] = useReducer(reduce, initialState);
  const [skins, setSkins] = useState<Skin[]>([]);
  const [defaultThumb, setDefaultThumb] = useState<string | null>(null);
  const [folderIcon, setFolderIcon] = useState<string | null>(null);
  const [platform, setPlatform] = useState<PlatformInfo>({ os: "macos", browse_label: "your Mac", note: "" });
  const [view, setView] = useState<View>("skins");
  /** The tag the library is filtered by; empty for all of them. */
  const [tag, setTag] = useState("");
  const [query, setQuery] = useState("");
  const [favorites, setFavorites] = useState<string[]>(() => loadFavorites());
  const [loadError, setLoadError] = useState<string | null>(null);
  const [aboutOpen, setAboutOpen] = useState(false);
  /** The skin whose Delete was pressed, while "are you sure?" is open. */
  const [confirmingDelete, setConfirmingDelete] = useState<Skin | null>(null);
  /** The skin whose ⋯ menu is open, the button it opened from, and whether the keyboard opened it. */
  const [menu, setMenu] = useState<{ skin: Skin; anchor: HTMLElement; keyboard: boolean } | null>(null);
  /** "Share with community" is open, for all of the user's skins or just one. */
  const [sharing, setSharing] = useState<{ only?: Skin } | null>(null);
  const [settingsTab, setSettingsTab] = useState<SettingsTab | null>(null);
  /** Bumped when a key is saved or removed in Settings, so the studio reads the providers again. */
  const [keysVersion, setKeysVersion] = useState(0);
  const { theme, pref: themePref, setPref: setThemePref, toggle: toggleThemePref } = useTheme();
  const { items: toastItems, push: toast, dismiss: dismissToast } = useToasts();
  const updates = useUpdates();

  useEffect(() => {
    api.platformInfo().then(setPlatform).catch(() => {});
    api
      .listSkins()
      .then((list) => {
        setSkins(newestFirst(list.skins));
        setDefaultThumb(list.default_thumbnail);
      })
      .catch((e) => setLoadError(errorMessage(e)));
  }, []);

  /** Puts skins in the library, or updates the ones already there. */
  const addSkins = useCallback((added: Skin[]) => {
    setSkins((prev) => newestFirst([...added, ...prev.filter((s) => !added.some((a) => a.id === s.id))]));
  }, []);
  const addSkin = useCallback((skin: Skin) => addSkins([skin]), [addSkins]);

  const yours = useMemo(() => skins.filter(isYours), [skins]);

  // The sidebar picks what is in view (everything, yours, favourites); the top bar narrows it to
  // one tag. Its filters are the tags in view, most used first.
  const inView = useMemo(
    () => (view === "yours" ? yours : view === "faves" ? skins.filter((s) => favorites.includes(s.id)) : skins),
    [view, yours, skins, favorites],
  );
  const tabs = useMemo<TabCount[]>(
    () => [{ id: "", label: "All", count: inView.length }, ...tagCounts(inView).map((t) => ({ id: t.tag, label: tagLabel(t.tag), count: t.count }))],
    [inView],
  );
  // A tag nothing in view carries any more (edited away, deleted) falls back to All.
  const activeTag = tabs.some((t) => t.id === tag) ? tag : "";

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return inView.filter(
      (s) => (!activeTag || s.tags.includes(activeTag)) && (!q || s.name.toLowerCase().includes(q) || s.tags.some((t) => t.includes(q))),
    );
  }, [inView, activeTag, query]);

  /** Opens the whole library filtered by one tag, as "Show" after adding a pack does. */
  const showTag = useCallback((t: string) => {
    setView("skins");
    setTag(t);
    setQuery("");
  }, []);

  const refreshFolderIcon = useCallback((path: string) => {
    api
      .folderIcon(path)
      .then((url) => setFolderIcon(url))
      .catch(() => setFolderIcon(null));
  }, []);

  const takePath = useCallback(
    async (path: string) => {
      try {
        const info = await api.inspectPath(path);
        if (info.kind === "folder") {
          setFolderIcon(null);
          dispatch({ type: "folderDropped", folder: { path: info.path, name: info.name } });
          refreshFolderIcon(info.path);
        } else if (info.kind === "image") {
          const skin = await api.importImage(info.path);
          addSkin(skin);
          setQuery("");
          setView("yours");
          dispatch({ type: "skinSelected", skinId: skin.id });
          toast(
            skin.kind === "folder"
              ? `${skin.name} is in Yours, background removed`
              : `${skin.name} is in Yours, wrapped onto a folder`,
            { tone: "ok" },
          );
        } else {
          dispatch({ type: "invalidDrop", message: "That isn't a folder or a picture." });
        }
      } catch (e) {
        dispatch({ type: "invalidDrop", message: errorMessage(e) });
      }
    },
    [refreshFolderIcon, addSkin, toast],
  );

  useDragDrop(
    useCallback((paths: string[]) => void (paths[0] && takePath(paths[0])), [takePath]),
    useCallback((info) => dispatch({ type: "drag", info }), []),
  );

  const browseFolder = useCallback(async () => {
    if (!isTauri()) return takePath("/Users/you/Documents/Projects");
    const picked = await open({ directory: true, multiple: false, title: "Choose a folder" }).catch(() => null);
    if (typeof picked === "string") await takePath(picked);
  }, [takePath]);

  const pickPhoto = useCallback(async () => {
    if (!isTauri()) return takePath("/Users/you/Pictures/Lighthouse at dusk.jpg");
    const picked = await open({
      multiple: false,
      title: "Choose a picture",
      filters: [{ name: "Pictures", extensions: IMAGE_EXTENSIONS }],
    }).catch(() => null);
    if (typeof picked === "string") await takePath(picked);
  }, [takePath]);

  const apply = useCallback(async () => {
    if (!state.folder || !state.skinId) return;
    dispatch({ type: "applyStarted" });
    try {
      await api.applySkin(state.folder.path, state.skinId);
      dispatch({ type: "applySucceeded" });
      refreshFolderIcon(state.folder.path);
    } catch (e) {
      dispatch({ type: "applyFailed", message: `Couldn't apply the skin: ${errorMessage(e)}` });
    }
  }, [state.folder, state.skinId, refreshFolderIcon]);

  const revert = useCallback(async () => {
    if (!state.folder) return;
    dispatch({ type: "revertStarted" });
    try {
      await api.revertSkin(state.folder.path);
      dispatch({ type: "revertSucceeded" });
      refreshFolderIcon(state.folder.path);
      toast(`${state.folder.name} has its default icon back`, { tone: "ok" });
    } catch (e) {
      dispatch({ type: "revertFailed", message: `Couldn't put the default icon back: ${errorMessage(e)}` });
    }
  }, [state.folder, refreshFolderIcon, toast]);

  const reveal = useCallback(() => {
    if (!state.folder) return;
    revealItemInDir(state.folder.path).catch((e) => toast(`Couldn't open it: ${errorMessage(e)}`, { tone: "danger" }));
  }, [state.folder, toast]);

  const onToggleFavorite = useCallback((id: string) => {
    setFavorites((prev) => {
      const next = toggleFavorite(prev, id);
      saveFavorites(next);
      return next;
    });
  }, []);

  const askDelete = useCallback((skin: Skin) => setConfirmingDelete(skin), []);
  const cancelDelete = useCallback(() => setConfirmingDelete(null), []);

  /** Deletes a skin once "are you sure?" is answered. It leaves the grid at once and comes back if the delete fails. */
  const deleteSkin = useCallback(
    (skin: Skin) => {
      setConfirmingDelete(null);
      if (state.skinId === skin.id) dispatch({ type: "skinCleared" });
      setSkins((prev) => prev.filter((s) => s.id !== skin.id));
      api
        .deleteSkin(skin.id)
        .then(() => {
          setFavorites((prev) => {
            const next = prev.filter((id) => id !== skin.id);
            saveFavorites(next);
            return next;
          });
          toast(`Deleted ${skin.name}`, { tone: "ok" });
        })
        .catch((e) => {
          setSkins((prev) => newestFirst([skin, ...prev.filter((s) => s.id !== skin.id)]));
          toast(`Couldn't delete ${skin.name}: ${errorMessage(e)}`, { tone: "danger" });
        });
    },
    [state.skinId, toast],
  );

  /** Shows the new name straight away, then the name as saved; the old one comes back if saving fails. */
  /** Shows the new name and tags straight away, then as saved; the old ones come back if saving fails. */
  const saveEdit = useCallback(
    (skin: Skin, name: string, tags: string[]) => {
      const show = (next: { name: string; tags: string[] }) =>
        setSkins((prev) => prev.map((s) => (s.id === skin.id ? { ...s, ...next } : s)));
      show({ name, tags });
      api
        .editSkin(skin.id, name, tags)
        .then(show)
        .catch((e) => {
          show({ name: skin.name, tags: skin.tags });
          toast(`Couldn't save ${skin.name}: ${errorMessage(e)}`, { tone: "danger" });
        });
    },
    [toast],
  );
  const allTags = useMemo(() => tagCounts(skins).map((t) => t.tag), [skins]);

  /** Takes skins out of the library after their pack was removed. */
  const dropSkins = useCallback(
    (ids: string[]) => {
      if (state.skinId && ids.includes(state.skinId)) dispatch({ type: "skinCleared" });
      setSkins((prev) => prev.filter((s) => !ids.includes(s.id)));
      setFavorites((prev) => {
        const next = prev.filter((id) => !ids.includes(id));
        saveFavorites(next);
        return next;
      });
    },
    [state.skinId],
  );

  /** A skin as the library has it now, for the studio's result cards. */
  const skinOf = useCallback((id: string) => skins.find((s) => s.id === id), [skins]);

  /** Opens a skin's ⋯ menu beside `anchor`; the same ⋯ clicked again closes it. */
  const openMenu = useCallback((skin: Skin, anchor: HTMLElement, keyboard = false) => {
    setMenu((open) => (open && open.skin.id === skin.id && open.anchor === anchor && !keyboard ? null : { skin, anchor, keyboard }));
  }, []);
  const closeMenu = useCallback(() => setMenu(null), []);
  const closeSharing = useCallback(() => setSharing(null), []);
  const closeSettings = useCallback(() => setSettingsTab(null), []);

  // About opens while the version badge (or the popover itself) is hovered, and closes a moment
  // after the pointer leaves both, so it can travel from one to the other.
  const aboutTimer = useRef(0);
  const hoverAbout = useCallback((inside: boolean) => {
    window.clearTimeout(aboutTimer.current);
    if (inside) setAboutOpen(true);
    else aboutTimer.current = window.setTimeout(() => setAboutOpen(false), 220);
  }, []);

  const selected = skins.find((s) => s.id === state.skinId) ?? null;
  const q = query.trim();
  const empty: Empty | null = loadError
    ? { icon: <FolderOpenIcon size={22} />, title: "The skins didn't load", text: loadError }
    : visible.length > 0
      ? null
      : q
        ? {
            icon: <SearchIcon size={20} />,
            title: `Nothing matches "${q}"`,
            text: "Try another word, or look in All.",
            action: (
              <button type="button" className="btn btn-secondary" onClick={() => setQuery("")}>
                Clear search
              </button>
            ),
          }
        : view === "faves"
          ? { icon: <StarIcon size={20} />, title: "No favourites yet", text: "Tap the star on any skin and it will wait for you here." }
          : view === "skins" && skins.length === 0
            ? {
                icon: <FolderOpenIcon size={22} />,
                title: "No skins yet",
                text: "Add a free pack from Community, bring a picture of your own, or have AI paint one.",
                action: (
                  <div className="empty-actions">
                    <button type="button" className="btn btn-primary" onClick={() => setView("community")}>
                      Browse packs
                    </button>
                    <button type="button" className="btn btn-secondary" onClick={pickPhoto}>
                      Add your photo
                    </button>
                  </div>
                ),
              }
            : null;

  const library = view === "skins" || view === "yours" || view === "faves";

  return (
    <main className={`app os-${platform.os}`}>
      <Sidebar
        view={view}
        onView={(v) => {
          setView(v);
          setTag("");
        }}
        favoritesCount={favorites.filter((id) => skins.some((s) => s.id === id)).length}
        yoursCount={yours.length}
        onImport={pickPhoto}
        theme={theme}
        onToggleTheme={toggleThemePref}
        onAboutHover={hoverAbout}
        aboutOpen={aboutOpen}
        updateReady={updates.status.state === "available"}
        onSettings={() => setSettingsTab("general")}
        settingsOpen={settingsTab !== null}
      />
      <AboutMenu
        note={platform.note}
        open={aboutOpen}
        onHover={hoverAbout}
        onClose={() => setAboutOpen(false)}
        updates={updates.status}
        onCheckUpdates={updates.check}
        onShowUpdate={updates.showDialog}
      />

      <section
        className={state.drag?.kind === "image" ? "island island-main is-drop-target" : "island island-main"}
        aria-label="library"
      >
        <span className="drop-glow" aria-hidden="true" />
        {library && (
          <>
            <GalleryToolbar tabs={tabs} active={activeTag} onChange={setTag} query={query} onQuery={setQuery} />
            <div className="gallery-scroll scroll-on-hover">
              <Gallery
                skins={visible}
                selectedId={state.skinId}
                favorites={favorites}
                empty={empty}
                animationKey={`${view}:${activeTag}:${q}`}
                onAdd={view === "yours" && !q && !activeTag ? pickPhoto : undefined}
                onSelect={(id) => dispatch({ type: "skinSelected", skinId: id })}
                onToggleFavorite={onToggleFavorite}
                onRemove={askDelete}
                onMenu={openMenu}
                menuFor={menu?.skin.id ?? null}
              />
            </div>
          </>
        )}
        {view === "community" && (
          <CommunityView onShare={() => setSharing({})} onAdded={addSkins} onRemoved={dropSkins} onShowTag={showTag} toast={toast} />
        )}
        {view === "generate" && (
          <Studio
            folderName={state.folder?.name ?? null}
            selectedId={state.skinId}
            onGenerated={addSkin}
            onTryOn={(id) => dispatch({ type: "skinSelected", skinId: id })}
            onImport={pickPhoto}
            skinOf={skinOf}
            onMenu={openMenu}
            keysVersion={keysVersion}
            toast={toast}
          />
        )}
      </section>

      <FolderStage
        state={state}
        skin={selected}
        folderIcon={folderIcon}
        defaultThumb={defaultThumb}
        os={platform.os}
        browseLabel={browseLabel(platform.os)}
        onBrowse={browseFolder}
        onApply={apply}
        onRevert={revert}
        onReveal={reveal}
      />

      {confirmingDelete && (
        <Confirm
          title={`Delete "${confirmingDelete.name}"?`}
          text="This removes it from your library for good. Folders that already use it keep their icon."
          image={confirmingDelete.thumbnail}
          action="Delete"
          onCancel={cancelDelete}
          onConfirm={() => deleteSkin(confirmingDelete)}
        />
      )}
      {menu && (
        <SkinMenu
          key={menu.skin.id}
          skin={menu.skin}
          anchor={menu.anchor}
          focusName={menu.keyboard}
          suggestions={allTags}
          onSave={(name, tags) => saveEdit(menu.skin, name, tags)}
          onShare={
            isYours(menu.skin)
              ? () => {
                  setMenu(null);
                  setSharing({ only: menu.skin });
                }
              : undefined
          }
          onDelete={() => {
            setMenu(null);
            askDelete(menu.skin);
          }}
          onClose={closeMenu}
        />
      )}
      {sharing && <SharePack yours={yours} only={sharing.only} fileBrowser={fileBrowser(platform.os)} onClose={closeSharing} />}
      {settingsTab && (
        <Settings
          tab={settingsTab}
          themePref={themePref}
          onThemePref={setThemePref}
          fileBrowser={fileBrowser(platform.os)}
          savedCount={skins.filter((s) => s.custom).length}
          note={platform.note}
          onKeysChanged={() => setKeysVersion((v) => v + 1)}
          onClose={closeSettings}
          toast={toast}
          updates={updates.status}
          onCheckUpdates={updates.check}
          onShowUpdate={updates.showDialog}
        />
      )}
      {updates.dialog && <UpdateDialog update={updates.dialog} onClose={updates.hideDialog} />}
      <Toaster items={toastItems} onDismiss={dismissToast} />
    </main>
  );
}
