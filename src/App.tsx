import { useCallback, useEffect, useMemo, useReducer, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, errorMessage, type PlatformInfo, type Skin } from "./lib/tauri";
import { isTauri } from "./lib/devMock";
import { IMAGE_EXTENSIONS } from "./lib/files";
import { browseLabel } from "./lib/platform";
import { initialState, reduce } from "./state/dropzone";
import { loadFavorites, saveFavorites, toggleFavorite } from "./state/favorites";
import { applyTheme, loadThemePref, resolveTheme, saveThemePref, toggleTheme, type Theme, type ThemePref } from "./state/theme";
import { useDragDrop } from "./hooks/useDragDrop";
import { useToasts } from "./hooks/useToasts";
import { Sidebar, type View } from "./components/Sidebar";
import { GalleryToolbar, type TabCount } from "./components/GalleryToolbar";
import { Gallery, type Empty } from "./components/Gallery";
import { FolderStage } from "./components/FolderStage";
import { AboutMenu } from "./components/AboutMenu";
import { CommunityView } from "./components/CommunityView";
import { Studio } from "./components/Studio";
import { ConfirmDelete } from "./components/ConfirmDelete";
import { Toaster } from "./components/Toaster";
import { SearchIcon } from "./components/icons/search";
import { StarIcon } from "./components/icons/star";
import { FolderOpenIcon } from "./components/icons/folder-open";

const ALL = "all";
const YOURS = "yours";
const FAVES = "faves";

function useTheme(): [Theme, () => void] {
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
  return [theme, toggle];
}

/** Newest of the user's skins first, then the built-ins in their manifest order. */
function withUserFirst(list: Skin[]): Skin[] {
  const mine = list.filter((s) => s.custom).sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));
  return [...mine, ...list.filter((s) => !s.custom)];
}

export default function App() {
  const [state, dispatch] = useReducer(reduce, initialState);
  const [skins, setSkins] = useState<Skin[]>([]);
  const [defaultThumb, setDefaultThumb] = useState<string | null>(null);
  const [folderIcon, setFolderIcon] = useState<string | null>(null);
  const [platform, setPlatform] = useState<PlatformInfo>({ os: "macos", browse_label: "your Mac", note: "" });
  const [view, setView] = useState<View>("skins");
  const [tab, setTab] = useState<string>(ALL);
  const [query, setQuery] = useState("");
  const [favorites, setFavorites] = useState<string[]>(() => loadFavorites());
  const [loadError, setLoadError] = useState<string | null>(null);
  const [aboutOpen, setAboutOpen] = useState(false);
  /** The skin whose Delete was pressed, while "are you sure?" is open. */
  const [confirmingDelete, setConfirmingDelete] = useState<Skin | null>(null);
  const [theme, toggleThemePref] = useTheme();
  const { items: toastItems, push: toast, dismiss: dismissToast } = useToasts();

  useEffect(() => {
    api.platformInfo().then(setPlatform).catch(() => {});
    api
      .listSkins()
      .then((list) => {
        setSkins(withUserFirst(list.skins));
        setDefaultThumb(list.default_thumbnail);
      })
      .catch((e) => setLoadError(errorMessage(e)));
  }, []);

  const addSkin = useCallback((skin: Skin) => {
    setSkins((prev) => withUserFirst([skin, ...prev.filter((s) => s.id !== skin.id)]));
  }, []);

  const yoursCount = useMemo(() => skins.filter((s) => s.custom).length, [skins]);

  const effectiveTab = view === "yours" ? YOURS : view === "faves" ? FAVES : tab;
  const tabs = useMemo<TabCount[]>(() => {
    const collections: string[] = [];
    for (const s of skins) if (!s.custom && !collections.includes(s.collection)) collections.push(s.collection);
    const list: TabCount[] = [{ id: ALL, label: "All", count: skins.length }];
    for (const c of collections)
      list.push({ id: c, label: c.charAt(0).toUpperCase() + c.slice(1), count: skins.filter((s) => s.collection === c).length });
    list.push({ id: YOURS, label: "Yours", count: yoursCount });
    const faves = favorites.filter((id) => skins.some((s) => s.id === id)).length;
    if (faves > 0 || view === "faves") list.push({ id: FAVES, label: "Faves", count: faves });
    return list;
  }, [skins, yoursCount, favorites, view]);

  const openTab = useCallback((id: string) => {
    if (id === YOURS) setView("yours");
    else if (id === FAVES) setView("faves");
    else {
      setView("skins");
      setTab(id);
    }
  }, []);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return skins.filter((s) => {
      if (effectiveTab === FAVES && !favorites.includes(s.id)) return false;
      if (effectiveTab === YOURS && !s.custom) return false;
      if (effectiveTab !== ALL && effectiveTab !== FAVES && effectiveTab !== YOURS && s.collection !== effectiveTab) return false;
      return !q || s.name.toLowerCase().includes(q) || s.collection.toLowerCase().includes(q);
    });
  }, [skins, effectiveTab, favorites, query]);

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
          setSkins((prev) => withUserFirst([skin, ...prev.filter((s) => s.id !== skin.id)]));
          toast(`Couldn't delete ${skin.name}: ${errorMessage(e)}`, { tone: "danger" });
        });
    },
    [state.skinId, toast],
  );

  /** Shows the new name straight away, then the name as saved; the old one comes back if saving fails. */
  const renameSkin = useCallback(
    (skin: Skin, name: string) => {
      const show = (to: string) => setSkins((prev) => prev.map((s) => (s.id === skin.id ? { ...s, name: to } : s)));
      show(name);
      api
        .renameSkin(skin.id, name)
        .then(show)
        .catch((e) => {
          show(skin.name);
          toast(`Couldn't rename ${skin.name}: ${errorMessage(e)}`, { tone: "danger" });
        });
    },
    [toast],
  );

  /** A skin's current name, for cards that keep their own copy of it (the studio's results). */
  const nameOf = useCallback((id: string) => skins.find((s) => s.id === id)?.name, [skins]);

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
        : effectiveTab === FAVES
          ? { icon: <StarIcon size={20} />, title: "No favourites yet", text: "Tap the star on any skin and it will wait for you here." }
          : null;

  const library = view === "skins" || view === "yours" || view === "faves";

  return (
    <main className={`app os-${platform.os}`}>
      <Sidebar
        view={view}
        onView={(v) => {
          setView(v);
          if (v === "skins") setTab(ALL);
        }}
        favoritesCount={favorites.filter((id) => skins.some((s) => s.id === id)).length}
        yoursCount={yoursCount}
        onImport={pickPhoto}
        theme={theme}
        onToggleTheme={toggleThemePref}
        onAbout={() => setAboutOpen((v) => !v)}
        aboutOpen={aboutOpen}
      />
      <AboutMenu note={platform.note} open={aboutOpen} onClose={() => setAboutOpen(false)} />

      <section
        className={state.drag?.kind === "image" ? "island island-main is-drop-target" : "island island-main"}
        aria-label="library"
      >
        <span className="drop-glow" aria-hidden="true" />
        {library && (
          <>
            <GalleryToolbar tabs={tabs} active={effectiveTab} onChange={openTab} query={query} onQuery={setQuery} />
            <div className="gallery-scroll">
              <Gallery
                skins={visible}
                selectedId={state.skinId}
                favorites={favorites}
                empty={empty}
                animationKey={`${effectiveTab}:${q}`}
                onAdd={effectiveTab === YOURS && !q ? pickPhoto : undefined}
                onSelect={(id) => dispatch({ type: "skinSelected", skinId: id })}
                onToggleFavorite={onToggleFavorite}
                onRemove={askDelete}
                onRename={renameSkin}
              />
            </div>
          </>
        )}
        {view === "community" && <CommunityView onImport={pickPhoto} />}
        {view === "generate" && (
          <Studio
            folderName={state.folder?.name ?? null}
            selectedId={state.skinId}
            onGenerated={addSkin}
            onTryOn={(id) => dispatch({ type: "skinSelected", skinId: id })}
            onImport={pickPhoto}
            nameOf={nameOf}
            onRename={renameSkin}
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

      {confirmingDelete && <ConfirmDelete skin={confirmingDelete} onCancel={cancelDelete} onDelete={() => deleteSkin(confirmingDelete)} />}
      <Toaster items={toastItems} onDismiss={dismissToast} />
    </main>
  );
}
