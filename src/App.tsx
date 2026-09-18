import { useCallback, useEffect, useMemo, useReducer, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errorMessage, type PlatformInfo, type Skin } from "./lib/tauri";
import { browseLabel, tabBarInset } from "./lib/platform";
import { initialState, reduce } from "./state/dropzone";
import { loadFavorites, saveFavorites, toggleFavorite } from "./state/favorites";
import { useDragDrop } from "./hooks/useDragDrop";
import { TabBar } from "./components/TabBar";
import { Gallery } from "./components/Gallery";
import { DropZone } from "./components/DropZone";
import { Wordmark } from "./components/Wordmark";
import { AboutMenu } from "./components/AboutMenu";

const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "heic"];
const ALL = "all";
const FAVES = "faves";

export default function App() {
  const [state, dispatch] = useReducer(reduce, initialState);
  const [builtin, setBuiltin] = useState<Skin[]>([]);
  const [customSkins, setCustomSkins] = useState<Skin[]>([]);
  const [defaultThumb, setDefaultThumb] = useState<string | null>(null);
  const [platform, setPlatform] = useState<PlatformInfo>({ os: "macos", browse_label: "your Mac", note: "" });
  const [tab, setTab] = useState<string>(ALL);
  const [favorites, setFavorites] = useState<string[]>(() => loadFavorites());
  const [loadError, setLoadError] = useState<string | null>(null);

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
  const collections = useMemo(() => {
    const seen: string[] = [];
    for (const s of builtin) if (!seen.includes(s.collection)) seen.push(s.collection);
    return seen;
  }, [builtin]);
  const tabs = useMemo(() => [ALL, ...collections, ...(favorites.length ? [FAVES] : [])], [collections, favorites.length]);
  const visible = useMemo(() => {
    if (tab === ALL) return skins;
    if (tab === FAVES) return skins.filter((s) => favorites.includes(s.id));
    return skins.filter((s) => s.collection === tab);
  }, [skins, tab, favorites]);
  useEffect(() => {
    if (!tabs.includes(tab)) setTab(ALL);
  }, [tabs, tab]);

  const takePath = useCallback(async (path: string) => {
    try {
      const info = await api.inspectPath(path);
      if (info.kind === "folder") {
        dispatch({ type: "folderDropped", folder: { path: info.path, name: info.name } });
      } else if (info.kind === "image") {
        const skin = await api.importImage(info.path);
        setCustomSkins((prev) => [skin, ...prev.filter((s) => s.id !== skin.id)]);
        setTab(ALL);
        dispatch({ type: "skinSelected", skinId: skin.id });
      } else {
        dispatch({ type: "invalidDrop", message: "that's not a folder or a picture" });
      }
    } catch (e) {
      dispatch({ type: "invalidDrop", message: errorMessage(e) });
    }
  }, []);

  useDragDrop(
    useCallback((paths: string[]) => void (paths[0] && takePath(paths[0])), [takePath]),
    useCallback((hover: boolean) => dispatch({ type: "drag", hover }), []),
  );

  const browseFolder = useCallback(async () => {
    const picked = await open({ directory: true, multiple: false, title: "Choose a folder" }).catch(() => null);
    if (typeof picked === "string") await takePath(picked);
  }, [takePath]);

  const pickPhoto = useCallback(async () => {
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
    } catch (e) {
      dispatch({ type: "applyFailed", message: `couldn't apply the skin: ${errorMessage(e)}` });
    }
  }, [state.folder, state.skinId]);

  const revert = useCallback(async () => {
    if (!state.folder) return;
    dispatch({ type: "revertStarted" });
    try {
      await api.revertSkin(state.folder.path);
      dispatch({ type: "revertSucceeded" });
    } catch (e) {
      dispatch({ type: "revertFailed", message: `couldn't put the default icon back: ${errorMessage(e)}` });
    }
  }, [state.folder]);

  const onToggleFavorite = useCallback((id: string) => {
    setFavorites((prev) => {
      const next = toggleFavorite(prev, id);
      saveFavorites(next);
      return next;
    });
  }, []);

  const selected = skins.find((s) => s.id === state.skinId) ?? null;
  const preview = state.phase === "folder" ? defaultThumb : (selected?.thumbnail ?? defaultThumb);
  const emptyMessage = loadError ?? (tab === FAVES && visible.length === 0 ? "no favourites yet — tap the star on a skin" : null);

  return (
    <main className={`app os-${platform.os}`}>
      <section className="left">
        <header className="topbar" data-tauri-drag-region>
          <div className="topbar-left" style={{ marginLeft: tabBarInset(platform.os) }}>
            <TabBar tabs={tabs} active={tab} onChange={setTab} />
            <button
              type="button"
              className="pill-btn icon-btn"
              title="use your own picture"
              aria-label="use your own picture"
              onMouseDown={(e) => e.preventDefault()}
              onClick={pickPhoto}
            >
              <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                <rect x="3" y="5" width="18" height="14" rx="3" fill="none" stroke="currentColor" strokeWidth="2.2" />
                <circle cx="8.5" cy="10" r="1.7" fill="currentColor" />
                <path d="M5 17l4.5-4.5 3 3 2.5-2.5L19 17" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinejoin="round" />
              </svg>
            </button>
          </div>
        </header>
        <div className="gallery-scroll">
          <Gallery
            skins={visible}
            selectedId={state.skinId}
            favorites={favorites}
            emptyMessage={emptyMessage}
            onSelect={(id) => dispatch({ type: "skinSelected", skinId: id })}
            onToggleFavorite={onToggleFavorite}
          />
        </div>
      </section>
      <section className="right">
        <div className="right-top" data-tauri-drag-region>
          <AboutMenu note={platform.note} />
        </div>
        <Wordmark />
        <DropZone
          state={state}
          browseLabel={browseLabel(platform.os)}
          thumbnail={preview}
          onBrowse={browseFolder}
          onApply={apply}
          onRevert={revert}
        />
      </section>
    </main>
  );
}
