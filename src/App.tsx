import { useCallback, useEffect, useMemo, useReducer, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errorMessage, type PlatformInfo, type Skin } from "./lib/tauri";
import { isTauri } from "./lib/devMock";
import { browseLabel } from "./lib/platform";
import { initialState, reduce } from "./state/dropzone";
import { loadFavorites, saveFavorites, toggleFavorite } from "./state/favorites";
import { applyTheme, loadThemePref, resolveTheme, saveThemePref, toggleTheme, type Theme, type ThemePref } from "./state/theme";
import { useDragDrop } from "./hooks/useDragDrop";
import { Sidebar, type View } from "./components/Sidebar";
import { GalleryToolbar, type TabCount } from "./components/GalleryToolbar";
import { Gallery } from "./components/Gallery";
import { DropZone } from "./components/DropZone";
import { AboutMenu } from "./components/AboutMenu";
import { CommunityView } from "./components/CommunityView";
import { GenerateView } from "./components/GenerateView";
import { IconImage } from "./components/icons";

const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "heic"];
const ALL = "all";
const FAVES = "faves";

function useTheme(): [Theme, () => void] {
  const [pref, setPref] = useState<ThemePref>(() => loadThemePref());
  const [theme, setTheme] = useState<Theme>(() => resolveTheme(loadThemePref()));
  useEffect(() => {
    const t = resolveTheme(pref);
    setTheme(t);
    applyTheme(t);
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

export default function App() {
  const [state, dispatch] = useReducer(reduce, initialState);
  const [builtin, setBuiltin] = useState<Skin[]>([]);
  const [customSkins, setCustomSkins] = useState<Skin[]>([]);
  const [defaultThumb, setDefaultThumb] = useState<string | null>(null);
  const [folderIcon, setFolderIcon] = useState<string | null>(null);
  const [platform, setPlatform] = useState<PlatformInfo>({ os: "macos", browse_label: "your Mac", note: "" });
  const [view, setView] = useState<View>("skins");
  const [tab, setTab] = useState<string>(ALL);
  const [query, setQuery] = useState("");
  const [favorites, setFavorites] = useState<string[]>(() => loadFavorites());
  const [loadError, setLoadError] = useState<string | null>(null);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [theme, toggleThemePref] = useTheme();

  useEffect(() => {
    api.platformInfo().then(setPlatform).catch(() => {});
    api
      .listSkins()
      .then((list) => {
        setBuiltin(list.skins);
        setDefaultThumb(list.default_thumbnail);
      })
      .catch((e) => setLoadError(`couldn't load the skins: ${errorMessage(e)}`));
  }, []);

  const skins = useMemo(() => [...customSkins, ...builtin], [customSkins, builtin]);
  const tabs = useMemo<TabCount[]>(() => {
    const collections: string[] = [];
    for (const s of skins) if (!collections.includes(s.collection)) collections.push(s.collection);
    const list: TabCount[] = [{ id: ALL, label: "All", count: skins.length }];
    for (const c of collections) list.push({ id: c, label: c, count: skins.filter((s) => s.collection === c).length });
    if (favorites.length) list.push({ id: FAVES, label: "Faves", count: favorites.length });
    return list;
  }, [skins, favorites]);
  const effectiveTab = view === "faves" ? FAVES : tab;
  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return skins.filter((s) => {
      if (effectiveTab === FAVES && !favorites.includes(s.id)) return false;
      if (effectiveTab !== ALL && effectiveTab !== FAVES && s.collection !== effectiveTab) return false;
      return !q || s.name.toLowerCase().includes(q) || s.collection.toLowerCase().includes(q);
    });
  }, [skins, effectiveTab, favorites, query]);
  useEffect(() => {
    if (!tabs.some((t) => t.id === tab)) setTab(ALL);
  }, [tabs, tab]);

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
          setCustomSkins((prev) => [skin, ...prev.filter((s) => s.id !== skin.id)]);
          setView("skins");
          setTab(ALL);
          setQuery("");
          dispatch({ type: "skinSelected", skinId: skin.id });
        } else {
          dispatch({ type: "invalidDrop", message: "that's not a folder or a picture" });
        }
      } catch (e) {
        dispatch({ type: "invalidDrop", message: errorMessage(e) });
      }
    },
    [refreshFolderIcon],
  );

  useDragDrop(
    useCallback((paths: string[]) => void (paths[0] && takePath(paths[0])), [takePath]),
    useCallback((hover: boolean) => dispatch({ type: "drag", hover }), []),
  );

  const browseFolder = useCallback(async () => {
    if (!isTauri()) return takePath("/Users/you/Desktop/readme");
    const picked = await open({ directory: true, multiple: false, title: "Choose a folder" }).catch(() => null);
    if (typeof picked === "string") await takePath(picked);
  }, [takePath]);

  const pickPhoto = useCallback(async () => {
    if (!isTauri()) return takePath("/Users/you/Pictures/photo.jpg");
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
      dispatch({ type: "applyFailed", message: `couldn't apply the skin: ${errorMessage(e)}` });
    }
  }, [state.folder, state.skinId, refreshFolderIcon]);

  const revert = useCallback(async () => {
    if (!state.folder) return;
    dispatch({ type: "revertStarted" });
    try {
      await api.revertSkin(state.folder.path);
      dispatch({ type: "revertSucceeded" });
      refreshFolderIcon(state.folder.path);
    } catch (e) {
      dispatch({ type: "revertFailed", message: `couldn't put the default icon back: ${errorMessage(e)}` });
    }
  }, [state.folder, refreshFolderIcon]);

  const onToggleFavorite = useCallback((id: string) => {
    setFavorites((prev) => {
      const next = toggleFavorite(prev, id);
      saveFavorites(next);
      return next;
    });
  }, []);

  const selected = skins.find((s) => s.id === state.skinId) ?? null;
  // folder / applied: what the folder really looks like right now; ready / applying: what it will look like.
  const preview =
    state.phase === "folder" || state.phase === "applied" || state.phase === "reverting"
      ? (folderIcon ?? defaultThumb)
      : (selected?.thumbnail ?? folderIcon ?? defaultThumb);
  const emptyMessage =
    loadError ??
    (visible.length === 0
      ? effectiveTab === FAVES && !query
        ? "No favourites yet — tap the star on a skin to keep it here."
        : "No skins match that search."
      : null);

  const photoButton = (
    <button
      type="button"
      className="btn btn-secondary"
      title="use your own picture as a skin"
      onMouseDown={(e) => e.preventDefault()}
      onClick={pickPhoto}
    >
      <IconImage />
      Your photo
    </button>
  );

  return (
    <main className={`app os-${platform.os}`}>
      <div className="blobs" aria-hidden="true">
        <span className="blob blob-a" />
        <span className="blob blob-b" />
      </div>
      <Sidebar
        view={view}
        onView={(v) => {
          setView(v);
          if (v === "skins") setTab(ALL);
        }}
        favoritesCount={favorites.length}
        theme={theme}
        onToggleTheme={toggleThemePref}
        onAbout={() => setAboutOpen((v) => !v)}
        aboutOpen={aboutOpen}
      />
      <AboutMenu note={platform.note} open={aboutOpen} onClose={() => setAboutOpen(false)} />
      <div className="frame">
        <div className="body">
          <section className="content" key={view}>
            {(view === "skins" || view === "faves") && (
              <>
                <GalleryToolbar
                  tabs={view === "faves" ? tabs.filter((t) => t.id === FAVES) : tabs.filter((t) => t.id !== FAVES)}
                  active={effectiveTab}
                  onChange={setTab}
                  query={query}
                  onQuery={setQuery}
                  actions={photoButton}
                />
                <div className="gallery-scroll">
                  <Gallery
                    skins={visible}
                    selectedId={state.skinId}
                    favorites={favorites}
                    emptyMessage={emptyMessage}
                    animationKey={`${effectiveTab}:${query}:${skins.length}`}
                    onSelect={(id) => dispatch({ type: "skinSelected", skinId: id })}
                    onToggleFavorite={onToggleFavorite}
                  />
                </div>
              </>
            )}
            {view === "community" && (
              <>
                <div className="toolbar toolbar-plain" data-tauri-drag-region>
                  <div className="toolbar-actions">{photoButton}</div>
                </div>
                <CommunityView onImport={pickPhoto} />
              </>
            )}
            {view === "generate" && (
              <>
                <div className="toolbar toolbar-plain" data-tauri-drag-region>
                  <div className="toolbar-actions">{photoButton}</div>
                </div>
                <GenerateView />
              </>
            )}
          </section>
          <aside className="panel">
            <div className="panel-head">
              <h1 className="headline">
                Give any folder <span className="mark">a skin.</span>
              </h1>
              <p className="subline">Drop a folder, pick a skin, press apply.</p>
            </div>
            <DropZone
              state={state}
              browseLabel={browseLabel(platform.os)}
              thumbnail={preview}
              onBrowse={browseFolder}
              onApply={apply}
              onRevert={revert}
            />
          </aside>
        </div>
      </div>
    </main>
  );
}
