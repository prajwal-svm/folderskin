import { lazy, Suspense, useCallback, useEffect, useMemo, useReducer, useRef, useState, type CSSProperties } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, errorMessage, type PlatformInfo, type Skin } from "./lib/tauri";
import { isTauri, mockPickFolder } from "./lib/devMock";
import { IMAGE_EXTENSIONS } from "./lib/files";
import { browseLabel, fileBrowser } from "./lib/platform";
import { isYours, tagCounts, tagLabel } from "./lib/tags";
import { activeCount, applyFilters, type Filters, loadSort, matchesQuery, NO_FILTERS, saveSort, type Sort, sortSkins } from "./lib/filters";
import { applyLabel, CONFIRM_ABOVE, folders, formatBytes, mergeRuns, runToast, type TreeProgress, type TreeRun } from "./lib/tree";
import { throttle } from "./lib/throttle";
import { initialState, reduce } from "./state/dropzone";
import { loadFavorites, saveFavorites, toggleFavorite } from "./state/favorites";
import { flushChats } from "./state/chatStore";
import { applyTheme, loadThemePref, resolveTheme, saveThemePref, toggleTheme, type Theme, type ThemePref } from "./state/theme";
import { columns, DEFAULT_LAYOUT, dragRight, dragSidebar, LEFT, loadLayout, RIGHT, saveLayout, type Layout } from "./state/layout";
import { IslandResizer } from "./components/IslandResizer";
import { useDragDrop } from "./hooks/useDragDrop";
import { useToasts } from "./hooks/useToasts";
import { useUpdates } from "./hooks/useUpdates";
import { usePalettes } from "./hooks/usePalettes";
import { Sidebar, type View } from "./components/Sidebar";
import { GalleryToolbar, type TabCount } from "./components/GalleryToolbar";
import { Gallery, type Empty } from "./components/Gallery";
import { FolderStage } from "./components/FolderStage";
import { FilterMenu } from "./components/FilterMenu";
import { AboutMenu } from "./components/AboutMenu";
import { WindowControls } from "./components/WindowControls";
import { CommunityView } from "./components/CommunityView";
import type { ApplyOutcome, ComposerHandle, ComposerRequest } from "./components/composer/Composer";
import type { StudioHandle } from "./components/studio/Studio";
import { Confirm } from "./components/Confirm";
import { SkinMenu } from "./components/SkinMenu";
import { SharePack } from "./components/SharePack";
import { Settings, type SettingsTab } from "./components/Settings";
import { Toaster } from "./components/Toaster";
import { UpdateDialog } from "./components/UpdateDialog";
import { SearchIcon } from "./components/icons/search";
import { StarIcon } from "./components/icons/star";
import { FolderOpenIcon } from "./components/icons/folder-open";
import { ListFilterIcon } from "./components/icons/list-filter";
import { clip } from "./lib/names";


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

/** The composer is loaded the first time it's opened, so the rest of the app starts without it. */
const Composer = lazy(() => import("./components/composer/Composer").then((m) => ({ default: m.Composer })));
/** The AI view likewise. */
const Studio = lazy(() => import("./components/studio/Studio").then((m) => ({ default: m.Studio })));

/** A question before a big run over a folder and its subfolders, answered through `resolve`. */
type TreeAsk = {
  kind: "apply" | "remove";
  folderName: string;
  /** Folders inside the chosen one. */
  inside: number;
  skin: Pick<Skin, "id" | "name" | "thumbnail"> | null;
  /** Disk space one folder's copy of the icon takes, once known. */
  bytes: number | null;
  resolve: (ok: boolean) => void;
};

/** How long a folder that replaced another shows its own icon before the selected skin goes on. */
const ARRIVAL_MS = 900;

/** How long the sidebar takes to fold or open (the grid's transition in shell.css). */
const FOLD_MS = 320;

/**
 * The window's columns as the user left them, and the window's width, which they're fitted to.
 * Saved a moment after each change; folding or opening the sidebar animates, dragging doesn't.
 */
function useLayout() {
  const [layout, setLayout] = useState<Layout>(loadLayout);
  const [width, setWidth] = useState(() => window.innerWidth);
  const [folding, setFolding] = useState(false);
  useEffect(() => {
    const onResize = () => setWidth(window.innerWidth);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  useEffect(() => {
    const t = window.setTimeout(() => saveLayout(layout), 250);
    return () => window.clearTimeout(t);
  }, [layout]);
  useEffect(() => {
    if (!folding) return;
    const t = window.setTimeout(() => setFolding(false), FOLD_MS + 40);
    return () => window.clearTimeout(t);
  }, [folding]);
  const toggleRail = useCallback(() => {
    setFolding(true);
    setLayout((l) => ({ ...l, rail: !l.rail }));
  }, []);
  // A drag that folds or opens the sidebar animates too, rather than jumping.
  const latest = useRef(layout);
  latest.current = layout;
  const resizeSidebar = useCallback((to: number) => {
    const next = dragSidebar(latest.current, to);
    if (next === latest.current) return;
    if (next.rail !== latest.current.rail) setFolding(true);
    latest.current = next;
    setLayout(next);
  }, []);
  return { layout, setLayout, width, folding, toggleRail, resizeSidebar };
}

/** Newest first. A pack keeps its own order: the app gives its first skin the newest time. */
function newestFirst(list: Skin[]): Skin[] {
  return [...list].sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));
}

export default function App() {
  const [state, dispatch] = useReducer(reduce, initialState);
  const [skins, setSkins] = useState<Skin[]>([]);
  const [defaultThumb, setDefaultThumb] = useState<string | null>(null);
  /**
   * A folder's icon as the OS draws it, which folder it is, and whether it's a custom one a revert
   * would take off; null `url` when the OS can't say.
   */
  const [folderIcon, setFolderIcon] = useState<{ path: string; url: string | null; custom: boolean } | null>(null);
  const [platform, setPlatform] = useState<PlatformInfo>({ os: "macos", browse_label: "your Mac", note: "" });
  const [view, setView] = useState<View>("skins");
  /** The tag the library is filtered by; empty for all of them. */
  const [tag, setTag] = useState("");
  const [query, setQuery] = useState("");
  /** The filters and order behind the filter button; the order is remembered, the filters aren't. */
  const [filters, setFilters] = useState<Filters>(NO_FILTERS);
  const [sort, setSort] = useState<Sort>(() => loadSort());
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
  /** The composer, once it has been opened: it stays mounted so a design survives a visit elsewhere. */
  const [composerOpened, setComposerOpened] = useState(false);
  /** The AI chat likewise, so its scroll, words and pictures wait for the user to come back. */
  const [studioOpened, setStudioOpened] = useState(false);
  /** The folder panel hidden from the AI chat by choice, though a folder is chosen. */
  const [aiPanelHidden, setAiPanelHidden] = useState(false);
  const composer = useRef<ComposerHandle>(null);
  /** Stop was pressed on a run over a folder and its subfolders. */
  const [stopping, setStopping] = useState(false);
  const [treeAsk, setTreeAsk] = useState<TreeAsk | null>(null);
  /** A skin the composer was asked to edit or remix. */
  const [composerRequest, setComposerRequest] = useState<ComposerRequest | null>(null);
  const requestSeq = useRef(0);
  const { theme, pref: themePref, setPref: setThemePref, toggle: toggleThemePref } = useTheme();
  const { layout, setLayout, width: windowWidth, folding, toggleRail, resizeSidebar } = useLayout();

  // ⌘\ (Ctrl+\ elsewhere) folds the sidebar and opens it again, as its button's tooltip says.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.key !== "\\" || document.querySelector(".modal-backdrop")) return;
      e.preventDefault();
      toggleRail();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggleRail]);
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

  // The stylesheet reserves room for the window's own controls on Windows (shell.css). window.rs
  // sets the same flag before the first paint so there is no reflow; this is what makes it right
  // whatever the webview did with that script.
  useEffect(() => {
    document.documentElement.dataset.os = platform.os;
  }, [platform.os]);

  // In full screen the Mac's traffic lights are gone, and so is the room the folded sidebar keeps
  // for them at its top (shell.css).
  useEffect(() => {
    if (!isTauri() || platform.os !== "macos") return;
    const win = getCurrentWindow();
    let live = true;
    const check = () =>
      void win
        .isFullscreen()
        .then((full) => live && document.documentElement.toggleAttribute("data-fullscreen", full))
        .catch(() => {});
    check();
    const off = win.onResized(check);
    return () => {
      live = false;
      void off.then((stop) => stop());
    };
  }, [platform.os]);

  // The assistant's chats are saved a moment after they change (chatStore.ts). Closing the window
  // saves what is still waiting first, so a picture that has just been made stays in its chat;
  // a save that hangs keeps the window open for a second at most. The browser preview only has
  // pagehide, which can't wait for it.
  useEffect(() => {
    const flush = () => void flushChats();
    window.addEventListener("pagehide", flush);
    const off = isTauri() ? getCurrentWindow().onCloseRequested(() => Promise.race([flushChats(), new Promise<void>((r) => setTimeout(r, 1000))])) : null;
    return () => {
      window.removeEventListener("pagehide", flush);
      void off?.then((stop) => stop());
    };
  }, []);

  /** Puts skins in the library, or updates the ones already there. */
  const addSkins = useCallback((added: Skin[]) => {
    setSkins((prev) => newestFirst([...added, ...prev.filter((s) => !added.some((a) => a.id === s.id))]));
  }, []);
  const addSkin = useCallback((skin: Skin) => addSkins([skin]), [addSkins]);

  const yours = useMemo(() => skins.filter(isYours), [skins]);
  const { palettes, reading } = usePalettes(skins);

  // The sidebar picks what is in view (everything, yours, favourites) and the filters narrow it;
  // the top bar then narrows it to one tag, its tabs being the tags left, most used first.
  const inView = useMemo(
    () => (view === "yours" ? yours : view === "faves" ? skins.filter((s) => favorites.includes(s.id)) : skins),
    [view, yours, skins, favorites],
  );
  const filterCtx = useMemo(() => ({ favourites: new Set(favorites), palettes, now: Date.now() }), [favorites, palettes]);
  const filtered = useMemo(() => applyFilters(inView, filters, filterCtx), [inView, filters, filterCtx]);
  const filtering = activeCount(filters) > 0;
  const tabs = useMemo<TabCount[]>(
    () => [{ id: "", label: "All", count: filtered.length }, ...tagCounts(filtered).map((t) => ({ id: t.tag, label: tagLabel(t.tag), count: t.count }))],
    [filtered],
  );
  // A tag nothing in view carries any more (edited away, deleted, filtered out) falls back to All.
  const activeTag = tabs.some((t) => t.id === tag) ? tag : "";

  const visible = useMemo(
    () => sortSkins(filtered.filter((s) => (!activeTag || s.tags.includes(activeTag)) && matchesQuery(s, query)), sort),
    [filtered, activeTag, query, sort],
  );

  const chooseSort = useCallback((next: Sort) => {
    setSort(next);
    saveSort(next);
  }, []);

  /** Opens the whole library filtered by one tag, as "Show" after adding a pack does. */
  const showTag = useCallback((t: string) => {
    setView("skins");
    setTag(t);
    setQuery("");
    setFilters(NO_FILTERS);
  }, []);

  const refreshFolderIcon = useCallback((path: string) => {
    api
      .folderIcon(path)
      .then((icon) => setFolderIcon({ path, url: icon.url, custom: icon.custom }))
      .catch(() => setFolderIcon({ path, url: null, custom: false }));
  }, []);

  const takePath = useCallback(
    async (path: string) => {
      try {
        const info = await api.inspectPath(path);
        if (info.kind === "folder") {
          setFolderIcon(null);
          dispatch({ type: "folderDropped", folder: { path: info.path, name: info.name } });
          refreshFolderIcon(info.path);
          // How many folders are inside, for "Include subfolders"; counted for this folder alone.
          const at = info.path;
          api
            .subfolderCount(at)
            .then((subfolders) => dispatch({ type: "subfoldersCounted", path: at, subfolders }))
            .catch(() => dispatch({ type: "subfoldersCounted", path: at, subfolders: null }));
        } else if (info.kind === "image") {
          const skin = await api.importImage(info.path);
          addSkin(skin);
          setQuery("");
          setView("yours");
          dispatch({ type: "skinSelected", skinId: skin.id });
          toast(
            skin.kind === "folder"
              ? `${clip(skin.name)} is in Yours, background removed`
              : `${clip(skin.name)} is in Yours, wrapped onto a folder`,
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

  /** A drop on the composer: a picture becomes a layer of the design, a folder the one to apply it to. */
  const dropOnComposer = useCallback(
    async (path: string) => {
      try {
        const info = await api.inspectPath(path);
        if (info.kind === "image") composer.current?.addImagePath(info.path);
        else await takePath(path);
      } catch (e) {
        toast(errorMessage(e), { tone: "danger" });
      }
    },
    [takePath, toast],
  );

  /** A drop on the AI view: a picture is one to paint from, a folder the one the pictures are for. */
  const studio = useRef<StudioHandle>(null);
  const dropOnStudio = useCallback(
    async (path: string) => {
      try {
        const info = await api.inspectPath(path);
        if (info.kind === "image") studio.current?.addReference(info.path);
        else await takePath(path);
      } catch (e) {
        toast(errorMessage(e), { tone: "danger" });
      }
    },
    [takePath, toast],
  );

  useDragDrop(
    useCallback(
      (paths: string[]) =>
        void (paths[0] && (view === "compose" ? dropOnComposer(paths[0]) : view === "generate" ? dropOnStudio(paths[0]) : takePath(paths[0]))),
      [takePath, dropOnComposer, dropOnStudio, view],
    ),
    useCallback((info) => dispatch({ type: "drag", info }), []),
  );

  // A folder that replaced another shows its own icon for a moment, then tries the skin on. The
  // moment starts once its icon is on screen (or, if the icon is slow, a little later anyway). A
  // folder with a custom icon waits instead, offering to try the skin on or to remove its icon.
  const iconShown = folderIcon !== null && folderIcon.path === state.folder?.path;
  const customIcon = iconShown && folderIcon.custom;
  useEffect(() => {
    if (!state.arriving || customIcon) return;
    const t = window.setTimeout(() => dispatch({ type: "arrived" }), iconShown ? ARRIVAL_MS : ARRIVAL_MS + 600);
    return () => window.clearTimeout(t);
  }, [state.arriving, state.folder?.path, iconShown, customIcon]);

  const browseFolder = useCallback(async () => {
    if (import.meta.env.DEV && !isTauri()) return takePath(mockPickFolder());
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

  // The latest state and library, for work that carries on after an await.
  const latestState = useRef(state);
  latestState.current = state;
  const latestSkins = useRef(skins);
  latestSkins.current = skins;

  /** Asks before a big run over a folder and its subfolders; resolves to the answer. */
  const askTree = useCallback(
    (ask: Omit<TreeAsk, "resolve" | "bytes">) =>
      new Promise<boolean>((resolve) => {
        setTreeAsk({ ...ask, bytes: null, resolve });
        if (ask.kind === "apply" && ask.skin) {
          api
            .treeBytes(ask.skin.id)
            .then((bytes) => setTreeAsk((a) => (a && a.resolve === resolve ? { ...a, bytes } : a)))
            .catch(() => {});
        }
      }),
    [],
  );
  const answerTree = (ok: boolean) => {
    treeAsk?.resolve(ok);
    setTreeAsk(null);
  };

  /**
   * Applies a skin to the chosen folder and every folder inside it, or to `only` those (carrying on
   * after a stop, or trying failures again, when `prev` is the run that left them). Resolves to
   * the run, or to what went wrong when no folder could be changed.
   */
  const applyTree = useCallback(
    async (skinId: string, only?: string[], prev?: TreeRun): Promise<TreeRun | { error: string }> => {
      const folder = latestState.current.folder;
      if (!folder) return { error: "Choose a folder first" };
      setStopping(false);
      dispatch({ type: "applyStarted" });
      const progress = throttle<TreeProgress>((p) => dispatch({ type: "treeProgress", progress: p }));
      try {
        const result = await api.applySkinTree(folder.path, skinId, only ?? null, progress.push);
        const run: TreeRun = prev ? mergeRuns(prev, { ...result, kind: "apply" }) : { ...result, kind: "apply" };
        if (run.changed.length === 0 && !run.stopped) {
          const error = `Couldn't apply the skin: ${run.failed[0]?.reason ?? "no folder could be changed"}`;
          dispatch({ type: "applyFailed", message: error });
          return { error };
        }
        dispatch({ type: "applySucceeded", run });
        refreshFolderIcon(folder.path);
        return run;
      } catch (e) {
        const error = `Couldn't apply the skin: ${errorMessage(e)}`;
        dispatch({ type: "applyFailed", message: error });
        return { error };
      } finally {
        progress.cancel();
        setStopping(false);
      }
    },
    [refreshFolderIcon],
  );

  /** Puts the default icon back on `only` those folders, or on every folder in the tree with an icon of its own. */
  const revertTree = useCallback(
    async (only: string[] | null, prev?: TreeRun): Promise<TreeRun | null> => {
      const folder = latestState.current.folder;
      if (!folder) return null;
      setStopping(false);
      dispatch({ type: "revertStarted" });
      const progress = throttle<TreeProgress>((p) => dispatch({ type: "treeProgress", progress: p }));
      try {
        const result = await api.revertSkinTree(folder.path, only, progress.push);
        const run: TreeRun = prev ? mergeRuns(prev, { ...result, kind: "revert" }) : { ...result, kind: "revert" };
        dispatch({ type: "revertSucceeded", run });
        refreshFolderIcon(folder.path);
        return run;
      } catch (e) {
        dispatch({ type: "revertFailed", message: `Couldn't put the default icons back: ${errorMessage(e)}` });
        return null;
      } finally {
        progress.cancel();
        setStopping(false);
      }
    },
    [refreshFolderIcon],
  );

  const apply = useCallback(async () => {
    const { folder, skinId, includeSubfolders, subfolders } = latestState.current;
    if (!folder || !skinId) return;
    const inside = includeSubfolders && subfolders ? subfolders.count : 0;
    if (inside > 0) {
      const skin = latestSkins.current.find((s) => s.id === skinId) ?? null;
      if (inside + 1 > CONFIRM_ABOVE && !(await askTree({ kind: "apply", folderName: folder.name, inside, skin }))) return;
      await applyTree(skinId);
      return;
    }
    dispatch({ type: "applyStarted" });
    try {
      await api.applySkin(folder.path, skinId);
      dispatch({ type: "applySucceeded" });
      refreshFolderIcon(folder.path);
    } catch (e) {
      dispatch({ type: "applyFailed", message: `Couldn't apply the skin: ${errorMessage(e)}` });
    }
  }, [askTree, applyTree, refreshFolderIcon]);

  // Where the user is, for runs that finish after they may have moved on.
  const viewNow = useRef(view);
  viewNow.current = view;
  const carryOnNow = useRef<() => void>(() => {});

  /**
   * An apply over the tree, said in a toast for when the folder panel that sums it up isn't in
   * view: a stopped run offers to carry on, one with failures to show which.
   */
  const treeOutcome = useCallback((run: TreeRun, folderName: string, skinName: string): ApplyOutcome => {
    const carry = run.stopped && run.remaining.length > 0 && run.changed.length > 0;
    return {
      ok: true,
      message: runToast(run, folderName, skinName),
      tone: carry || run.failed.length > 0 ? "info" : "ok",
      action: carry
        ? { label: "Carry on", run: () => carryOnNow.current() }
        : run.failed.length > 0
          ? { label: "See which", run: () => setView("yours") }
          : undefined,
    };
  }, []);

  /** Carries a stopped run on to the folders it didn't reach. */
  const carryOn = useCallback(async () => {
    const { run, skinId, folder } = latestState.current;
    if (!run?.stopped || run.remaining.length === 0 || !folder) return;
    if (run.kind === "revert") {
      void revertTree(run.remaining, run);
      return;
    }
    if (!skinId) return;
    const next = await applyTree(skinId, run.remaining, run);
    // Carried on from the composer's toast: say how it ended there too.
    if (viewNow.current !== "compose") return;
    if ("error" in next) {
      toast(next.error, { tone: "danger" });
      return;
    }
    const said = treeOutcome(next, folder.name, latestSkins.current.find((s) => s.id === skinId)?.name ?? "the skin");
    toast(said.message ?? "", { tone: said.tone, action: said.action });
  }, [applyTree, revertTree, treeOutcome, toast]);
  carryOnNow.current = () => void carryOn();

  /** Tries the folders the last run couldn't change once more. */
  const tryAgain = useCallback(() => {
    const { run, skinId } = latestState.current;
    if (!run || run.failed.length === 0) return;
    const paths = run.failed.map((f) => f.path);
    if (run.kind === "apply" && skinId) void applyTree(skinId, paths, run);
    else if (run.kind === "revert") void revertTree(paths, run);
  }, [applyTree, revertTree]);

  const stopRun = useCallback(() => {
    setStopping(true);
    api.stopTreeRun().catch(() => {});
  }, []);

  // The composer's Save & apply: the same apply as the folder panel's, subfolders included when
  // they are, for a skin it just saved. Resolves to what happened, for the composer to say.
  const applyFromComposer = useCallback(
    async (skin: Skin): Promise<ApplyOutcome> => {
      const { folder, phase, includeSubfolders, subfolders } = latestState.current;
      if (!folder || phase === "applying" || phase === "reverting") return { ok: false };
      dispatch({ type: "skinSelected", skinId: skin.id });
      const inside = includeSubfolders && subfolders ? subfolders.count : 0;
      if (inside > 0) {
        if (inside + 1 > CONFIRM_ABOVE && !(await askTree({ kind: "apply", folderName: folder.name, inside, skin }))) return { ok: false };
        const run = await applyTree(skin.id);
        return "error" in run ? { ok: false, message: run.error, tone: "danger" } : treeOutcome(run, folder.name, skin.name);
      }
      dispatch({ type: "applyStarted" });
      try {
        await api.applySkin(folder.path, skin.id);
        dispatch({ type: "applySucceeded" });
        refreshFolderIcon(folder.path);
        return { ok: true };
      } catch (e) {
        const message = `Couldn't apply the skin: ${errorMessage(e)}`;
        dispatch({ type: "applyFailed", message });
        return { ok: false, message, tone: "danger" };
      }
    },
    [askTree, applyTree, treeOutcome, refreshFolderIcon],
  );

  /** A design saved from the composer: a new skin, or one in place of the design it changed. */
  const onComposerSaved = useCallback((skin: Skin, replaced: string | null) => {
    setSkins((prev) => {
      const rest = prev.filter((s) => s.id !== skin.id);
      const at = replaced ? rest.findIndex((s) => s.id === replaced) : -1;
      if (at < 0) return newestFirst([skin, ...rest]);
      const next = [...rest];
      next[at] = skin;
      return newestFirst(next);
    });
    if (replaced && replaced !== skin.id) {
      setFavorites((prev) => {
        if (!prev.includes(replaced)) return prev;
        const next = prev.map((id) => (id === replaced ? skin.id : id));
        saveFavorites(next);
        return next;
      });
    }
    dispatch({ type: "skinSelected", skinId: skin.id });
  }, []);

  /** Opens a skin in the composer: a design to edit again, anything else to remix. */
  const designSkin = useCallback((skin: Skin) => {
    requestSeq.current += 1;
    setComposerRequest({ kind: skin.source === "composer" ? "edit" : "remix", skin, nonce: requestSeq.current });
    setComposerOpened(true);
    setView("compose");
  }, []);

  useEffect(() => {
    if (view === "compose") setComposerOpened(true);
    if (view === "generate") setStudioOpened(true);
  }, [view]);

  const revert = useCallback(async () => {
    const { folder, includeSubfolders, subfolders, run, phase } = latestState.current;
    if (!folder) return;
    const inside = includeSubfolders && subfolders ? subfolders.count : 0;
    if (inside > 0) {
      // Undoing an apply over the tree takes off exactly what it put on. Anything else clears
      // every icon in the tree, which asks first.
      // A run stopped before it changed anything leaves only the folder's own earlier apply.
      const undo = phase === "applied" && run?.kind === "apply" ? (run.changed.length > 0 ? run.changed : [folder.path]) : null;
      if (!undo && !(await askTree({ kind: "remove", folderName: folder.name, inside, skin: null }))) return;
      await revertTree(undo);
      return;
    }
    dispatch({ type: "revertStarted" });
    try {
      await api.revertSkin(folder.path);
      dispatch({ type: "revertSucceeded" });
      refreshFolderIcon(folder.path);
      toast(`${clip(folder.name)} has its default icon back`, { tone: "ok" });
    } catch (e) {
      dispatch({ type: "revertFailed", message: `Couldn't put the default icon back: ${errorMessage(e)}` });
    }
  }, [askTree, revertTree, refreshFolderIcon, toast]);

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
          toast(`Deleted ${clip(skin.name)}`, { tone: "ok" });
        })
        .catch((e) => {
          setSkins((prev) => newestFirst([skin, ...prev.filter((s) => s.id !== skin.id)]));
          toast(`Couldn't delete ${clip(skin.name)}: ${errorMessage(e)}`, { tone: "danger" });
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
          toast(`Couldn't save ${clip(skin.name)}: ${errorMessage(e)}`, { tone: "danger" });
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
  // The icon of the folder on show, never one that arrived late for a folder picked before it.
  const stageIcon = folderIcon && folderIcon.path === state.folder?.path ? (folderIcon.url ?? defaultThumb) : undefined;
  const q = query.trim();
  const empty: Empty | null = loadError
    ? { icon: <FolderOpenIcon size={22} />, title: "The skins didn't load", text: loadError }
    : visible.length > 0
      ? null
      : filtering
        ? {
            icon: <ListFilterIcon size={20} />,
            title: q ? `Nothing matches "${q}" with these filters` : "No skins match these filters",
            text: "Take a filter or two off, or clear them all.",
            action: (
              <div className="empty-actions">
                <button type="button" className="btn btn-secondary" onClick={() => setFilters(NO_FILTERS)}>
                  Clear filters
                </button>
                {q && (
                  <button type="button" className="btn btn-secondary" onClick={() => setQuery("")}>
                    Clear search
                  </button>
                )}
              </div>
            ),
          }
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
  const composing = view === "compose";
  // Windows has no system caption bar (window.rs builds the window undecorated), so the folder
  // island carries the window's controls and a strip to drag it by.
  const windowsChrome = platform.os === "windows";
  // The AI chat has the window to itself until there's a folder to show: one chosen (and not hidden
  // on purpose), or one being dragged in, which needs somewhere to land.
  const aiView = view === "generate";
  const rightShown = !aiView || state.drag?.kind === "folder" || (state.folder !== null && !aiPanelHidden);
  const cols = columns(layout, windowWidth, rightShown);
  // The panel slides in and out rather than jumping; dragging an edge or resizing the window doesn't wait.
  const [sliding, setSliding] = useState(false);
  const lastShown = useRef(rightShown);
  useEffect(() => {
    if (lastShown.current === rightShown) return;
    lastShown.current = rightShown;
    setSliding(true);
    const t = window.setTimeout(() => setSliding(false), 460);
    return () => window.clearTimeout(t);
  }, [rightShown]);
  // A newly chosen folder is shown, even in the AI chat after its panel was hidden.
  useEffect(() => setAiPanelHidden(false), [state.folder?.path]);
  const full = columns(layout, windowWidth, true);

  return (
    <main
      className={`app os-${platform.os}${layout.rail ? " is-rail" : ""}${folding || sliding ? " is-folding" : ""}${rightShown ? "" : " is-right-off"}`}
      style={{ "--left-w": `${cols.left}px`, "--right-w": `${cols.right}px`, "--right-full": `${full.right}px` } as CSSProperties}
    >
      <IslandResizer
        label="sidebar width"
        className="is-left"
        width={cols.left}
        min={LEFT.min}
        max={LEFT.max}
        grows="right"
        onWidth={resizeSidebar}
        onReset={() => setLayout((l) => ({ ...l, rail: false, left: DEFAULT_LAYOUT.left }))}
      />
      {rightShown && (
      <IslandResizer
        label={composing ? "layers and settings width" : "folder panel width"}
        className="is-right"
        width={cols.right}
        min={RIGHT.min}
        max={RIGHT.max}
        grows="left"
        onWidth={(to) => setLayout((l) => dragRight(l, to, windowWidth))}
        onReset={() => setLayout((l) => ({ ...l, right: null }))}
      />
      )}
      <Sidebar
        view={view}
        onView={(v) => {
          setView(v);
          setTag("");
          setFilters(NO_FILTERS);
        }}
        skinsCount={skins.length}
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
        rail={layout.rail}
        onToggleRail={toggleRail}
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
      {windowsChrome && (
        <div className="winbar">
          {/* A sibling of the buttons, never their parent: a drag region swallows the mousedown of
              anything inside it, which would leave the controls looking live but doing nothing. */}
          <span className="winbar-drag" data-tauri-drag-region />
          <WindowControls />
        </div>
      )}

      {!composing && (
      <section
        className={state.drag?.kind === "image" ? "island island-main is-drop-target" : "island island-main"}
        aria-label="library"
      >
        <span className="drop-glow" aria-hidden="true" />
        {library && (
          <>
            <GalleryToolbar
              tabs={tabs}
              active={activeTag}
              onChange={setTag}
              query={query}
              onQuery={setQuery}
              extra={
                <FilterMenu
                  skins={inView}
                  filters={filters}
                  onFilters={setFilters}
                  sort={sort}
                  onSort={chooseSort}
                  ctx={filterCtx}
                  showFavourites={view !== "faves"}
                  reading={reading}
                />
              }
            />
            <div className="gallery-scroll">
              <Gallery
                skins={visible}
                selectedId={state.skinId}
                favorites={favorites}
                empty={empty}
                animationKey={`${view}:${activeTag}:${q}:${JSON.stringify(filters)}:${sort}`}
                onAdd={view === "yours" && !q && !activeTag && !filtering ? pickPhoto : undefined}
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
        {(studioOpened || aiView) && (
          <Suspense fallback={null}>
            <Studio
              ref={studio}
              active={aiView}
              os={platform.os}
              folder={state.folder}
              shownId={state.skinId}
              appliedId={state.appliedSkinId}
              panelShown={rightShown}
              onTogglePanel={() => setAiPanelHidden(rightShown)}
              onChooseFolder={browseFolder}
              onUseFolder={(path) => {
                if (path) void takePath(path);
                else dispatch({ type: "folderCleared" });
              }}
              onPreview={(id) => dispatch({ type: "skinSelected", skinId: id })}
              onApply={applyFromComposer}
              onGenerated={addSkin}
              onImport={pickPhoto}
              skinOf={skinOf}
              onMenu={openMenu}
              keysVersion={keysVersion}
              dragImage={aiView && state.drag?.kind === "image"}
              toast={toast}
            />
          </Suspense>
        )}
      </section>
      )}

      {(composerOpened || composing) && (
        <Suspense
          fallback={
            composing ? (
              <>
                <section className="island island-main" aria-busy="true" />
                <aside className="island" aria-busy="true" />
              </>
            ) : null
          }
        >
        <Composer
          ref={composer}
          active={composing}
          skins={skins}
          folder={state.folder}
          folderIcon={stageIcon}
          applying={state.phase === "applying"}
          drag={composing ? state.drag : null}
          subfolders={state.subfolders}
          includeSubfolders={state.includeSubfolders}
          onIncludeSubfolders={(on) => dispatch({ type: "includeSubfolders", on })}
          progress={state.progress}
          stopping={stopping}
          onStop={stopRun}
          onChooseFolder={browseFolder}
          onSaved={onComposerSaved}
          onApply={applyFromComposer}
          request={composerRequest}
          toast={toast}
        />
        </Suspense>
      )}

      {!composing && (
      // The folder island keeps its width while its column opens and closes, so it slides in
      // from the window's edge rather than squeezing.
      <div className="right-slot" inert={!rightShown} aria-hidden={!rightShown}>
      <FolderStage
        state={state}
        skin={selected}
        folderIcon={stageIcon}
        customIcon={customIcon}
        defaultThumb={defaultThumb}
        os={platform.os}
        browseLabel={browseLabel(platform.os)}
        onBrowse={browseFolder}
        onApply={apply}
        onTryOn={() => dispatch({ type: "arrived" })}
        onRevert={revert}
        onReveal={reveal}
        stopping={stopping}
        onIncludeSubfolders={(on) => dispatch({ type: "includeSubfolders", on })}
        onStop={stopRun}
        onCarryOn={carryOn}
        onTryAgain={tryAgain}
        onDismissRun={() => dispatch({ type: "runDismissed" })}
        pickHint={aiView ? "Preview a picture from the chat" : undefined}
      />
      </div>
      )}

      {confirmingDelete && (
        <Confirm
          title={`Delete "${clip(confirmingDelete.name)}"?`}
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
          onDesign={() => {
            setMenu(null);
            designSkin(menu.skin);
          }}
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
      {treeAsk && (
        <Confirm
          title={
            treeAsk.kind === "apply"
              ? `Apply ${treeAsk.skin ? clip(treeAsk.skin.name) : "this skin"} to ${folders(treeAsk.inside + 1)}?`
              : `Remove the custom icons from ${folders(treeAsk.inside + 1)}?`
          }
          text={
            treeAsk.kind === "apply"
              ? `${clip(treeAsk.folderName)} and the ${folders(treeAsk.inside)} inside it get this skin, replacing any icon they have now. Each folder keeps its own copy of the icon${
                  treeAsk.bytes ? `, about ${formatBytes(treeAsk.bytes)}, so about ${formatBytes(treeAsk.bytes * (treeAsk.inside + 1))} in all` : ""
                }. Revert takes them all off again.`
              : `${clip(treeAsk.folderName)} and every folder inside it go back to the default folder icon, including icons they were given outside FolderSkin. Folders without one are left as they are.`
          }
          image={treeAsk.kind === "apply" ? treeAsk.skin?.thumbnail : undefined}
          action={treeAsk.kind === "apply" ? applyLabel(treeAsk.inside) : "Remove the icons"}
          tone={treeAsk.kind === "apply" ? "primary" : "danger"}
          onCancel={() => answerTree(false)}
          onConfirm={() => answerTree(true)}
        />
      )}
      {sharing && <SharePack yours={yours} only={sharing.only} fileBrowser={fileBrowser(platform.os)} onClose={closeSharing} />}
      {settingsTab && (
        <Settings
          tab={settingsTab}
          themePref={themePref}
          onThemePref={setThemePref}
          rail={layout.rail}
          onRail={(on) => on !== layout.rail && toggleRail()}
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
