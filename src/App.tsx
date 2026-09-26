import { lazy, Suspense, useCallback, useEffect, useMemo, useReducer, useRef, useState, type CSSProperties } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, errorMessage, type PlatformInfo, type Skin } from "./lib/tauri";
import { explain } from "./lib/sentences";
import { isTauri, mockPickFolder } from "./lib/devMock";
import { IMAGE_EXTENSIONS } from "./lib/files";
import { browseLabel } from "./lib/platform";
import { t as tNow, useT } from "./i18n";
import { isYours, tagCounts, tagLabel } from "./lib/tags";
import { activeCount, applyFilters, type Filters, loadSort, matchesQuery, NO_FILTERS, saveSort, type Sort, sortSkins } from "./lib/filters";
import { applyLabel, both, canCarryOn, CONFIRM_ABOVE, formatBytes, isBig, runNotice, runToast, treeProgress, type TreeRun, type TreeRunEvent } from "./lib/tree";
import { forRun } from "./lib/folderChoice";
import { useChoiceCount } from "./hooks/useChoiceCount";
import { community } from "./lib/communityStore";
import { watchInstallLinks } from "./lib/installLinks";
import { initialState, insideCount, insideCounted, isTree, reduce, treeChoice, type RunInfo, type State } from "./state/dropzone";
import { treeRuns, useTreeRun } from "./state/treeRun";
import { loadFavorites, saveFavorites, toggleFavorite } from "./state/favorites";
import { flushChats } from "./state/chatStore";
import { applyTheme, loadThemePref, resolveTheme, saveThemePref, toggleTheme, type Theme, type ThemePref } from "./state/theme";
import { chooseLook } from "./state/look";
import type { FolderStyle } from "./composer/parts";
import { columns, DEFAULT_LAYOUT, dragRight, dragSidebar, LEFT, loadLayout, RAIL, RIGHT, saveLayout, stepSidebar, type Layout } from "./state/layout";
import { IslandResizer } from "./components/IslandResizer";
import { LoaderIcon } from "./components/icons/loader";
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
import { RunDock } from "./components/RunDock";
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
  /** The folder's path, which decides how much room the icons take (see `treeBytes`). */
  folderPath: string;
  /** Folders inside the chosen one that the run takes, as counted so far. */
  inside: number;
  /** That's all of them: the count is done. */
  counted: boolean;
  /** They're the ones chosen in "Choose subfolders", not every folder inside. */
  chosen: boolean;
  skin: Pick<Skin, "id" | "name" | "thumbnail"> | null;
  /** Disk space one folder's copy of the icon takes, once known. */
  bytes: number | null;
  resolve: (ok: boolean) => void;
};

/** A run as the folder panel's state machine needs it. */
const runInfo = (run: TreeRun): RunInfo => ({ id: run.id, kind: run.kind, skinId: run.skin_id, changed: run.changed });

/** The question before a run over a tree: how many folders, "12,401+" while they're still being counted. */
function askTitle(ask: TreeAsk): string {
  const count = ask.inside + 1;
  if (ask.kind === "remove") return ask.counted ? tNow("folder.ask.removeTitle", { count }) : tNow("folder.ask.removeTitleCounting", { count });
  const skin = ask.skin ? clip(ask.skin.name) : tNow("folder.ask.thisSkin");
  return ask.counted ? tNow("folder.ask.applyTitle", { skin, count }) : tNow("folder.ask.applyTitleCounting", { skin, count });
}

/**
 * What the question before a run over a tree says. A big run, past `BIG_RUN` folders or still
 * being counted, says how many so far, the disk its icons take, and that it goes on in the
 * background.
 */
function askText(ask: TreeAsk): string {
  const folder = clip(ask.folderName);
  const count = ask.inside;
  const big = isBig(ask.inside, ask.counted);
  const background = tNow("folder.ask.background");
  if (ask.kind === "remove") {
    const text = !ask.chosen
      ? tNow("folder.ask.removeText", { folder })
      : ask.counted
        ? tNow("folder.ask.removeTextChosen", { folder, count })
        : tNow("folder.ask.removeTextChosenCounting", { folder });
    return big ? both(text, background) : text;
  }
  const sized = ask.bytes ? { size: formatBytes(ask.bytes), total: formatBytes(ask.bytes * (ask.inside + 1)) } : null;
  if (!big) {
    if (sized) return tNow(ask.chosen ? "folder.ask.applyTextChosenSized" : "folder.ask.applyTextSized", { folder, count, ...sized });
    return tNow(ask.chosen ? "folder.ask.applyTextChosen" : "folder.ask.applyText", { folder, count });
  }
  const goes = ask.counted
    ? tNow(ask.chosen ? "folder.ask.bigGoesChosen" : "folder.ask.bigGoes", { folder, count })
    : tNow(ask.chosen ? "folder.ask.bigGoesChosenCounting" : "folder.ask.bigGoesCounting", { folder, count });
  const size = sized ? tNow(ask.counted ? "folder.ask.bigSize" : "folder.ask.bigSizeCounting", sized) : null;
  return both(size ? both(goes, size) : goes, background);
}

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
  // A drag or an arrow key that folds or opens the sidebar animates too, rather than jumping.
  const latest = useRef(layout);
  latest.current = layout;
  const moveSidebar = useCallback((next: Layout) => {
    if (next === latest.current) return;
    if (next.rail !== latest.current.rail) setFolding(true);
    latest.current = next;
    setLayout(next);
  }, []);
  const resizeSidebar = useCallback((to: number) => moveSidebar(dragSidebar(latest.current, to)), [moveSidebar]);
  const stepSidebarEdge = useCallback((from: number, by: number) => moveSidebar(stepSidebar(latest.current, from, by)), [moveSidebar]);
  return { layout, setLayout, width, folding, toggleRail, resizeSidebar, stepSidebarEdge };
}

/** Newest first. A pack keeps its own order: the app gives its first skin the newest time. */
function newestFirst(list: Skin[]): Skin[] {
  return [...list].sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));
}

export default function App() {
  const t = useT();
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
  const [treeAsk, setTreeAsk] = useState<TreeAsk | null>(null);
  /** The latest run over a folder's tree, going on in the background whatever the window shows. */
  const run = useTreeRun();
  /** A skin the composer was asked to edit or remix. */
  const [composerRequest, setComposerRequest] = useState<ComposerRequest | null>(null);
  const requestSeq = useRef(0);
  const { theme, pref: themePref, setPref: setThemePref, toggle: toggleThemePref } = useTheme();
  const { layout, setLayout, width: windowWidth, folding, toggleRail, resizeSidebar, stepSidebarEdge } = useLayout();

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

  // A run over a folder's tree goes on in the background, and so does counting the folders inside
  // the one on show: the app says how they're going, heard here whatever the window shows.
  useEffect(() => {
    api.treeRun().then(treeRuns.take).catch(() => {});
    const stopRuns = api.onTreeRun(treeRuns.take);
    const stopCounts = api.onSubfolderCount((count) =>
      dispatch({ type: "subfoldersCounted", path: count.folder, subfolders: { count: count.count, done: count.done } }),
    );
    return () => {
      stopRuns();
      stopCounts();
    };
  }, []);

  // Skins go on the Mac's folder or Windows' (state/look.ts). After a switch the library's
  // thumbnails and the plain folder are drawn again on the new one.
  /** The folder the library's thumbnails are being drawn on again, until they arrive. */
  const [redrawing, setRedrawing] = useState<FolderStyle | null>(null);
  const lookRun = useRef(0);
  const chooseFolderLook = useCallback(
    (look: FolderStyle) => {
      // Every thumbnail is drawn again, which takes a moment; until then the old ones are dimmed
      // and a note says so. Only the last switch's thumbnails are kept, if two cross.
      const run = ++lookRun.current;
      setRedrawing(look);
      chooseLook(look)
        .then(() => api.listSkins())
        .then((list) => {
          if (run !== lookRun.current) return;
          setSkins(newestFirst(list.skins));
          setDefaultThumb(list.default_thumbnail);
        })
        .catch((e) => toast(tNow("folder.errors.switchLook", { reason: errorMessage(e) }), { tone: "danger" }))
        .finally(() => {
          if (run === lookRun.current) setRedrawing(null);
        });
    },
    [toast],
  );

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
    () => [{ id: "", label: t("library.tabs.all"), count: filtered.length }, ...tagCounts(filtered).map((c) => ({ id: c.tag, label: tagLabel(c.tag), count: c.count }))],
    [filtered, t],
  );
  // A tag nothing in view carries any more (edited away, deleted, filtered out) falls back to All.
  const activeTag = tabs.some((t) => t.id === tag) ? tag : "";
  // The search box says where it looks: in the tag picked, so nobody takes it for a search of everything.
  const activeTagName = activeTag ? (tabs.find((tab) => tab.id === activeTag)?.label ?? tagLabel(activeTag)) : "";

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

  // A folderskin://install link, from an Install button on folderskin.app: Community, open on that
  // pack and adding it as its Add button does (lib/installLinks.ts, lib/communityStore.ts). The app
  // has brought the window forward already.
  useEffect(
    () =>
      watchInstallLinks((packId) => {
        setView("community");
        setTag("");
        setFilters(NO_FILTERS);
        community.install(packId);
      }),
    [],
  );

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
          // How many folders are inside, for "Include subfolders": counted in the background for
          // this folder alone, the count before it abandoned. A small folder is counted by the
          // time this answers, and a big one goes on growing (the listener above).
          const at = info.path;
          api
            .subfolderCount(at)
            .then((count) => dispatch({ type: "subfoldersCounted", path: at, subfolders: { count: count.count, done: count.done } }))
            .catch(() => dispatch({ type: "subfoldersCounted", path: at, subfolders: null }));
        } else if (info.kind === "image") {
          const skin = await api.importImage(info.path);
          addSkin(skin);
          setQuery("");
          setView("yours");
          dispatch({ type: "skinSelected", skinId: skin.id });
          toast(
            skin.kind === "folder"
              ? tNow("library.toast.importedFolder", { name: clip(skin.name) })
              : tNow("library.toast.importedArtwork", { name: clip(skin.name) }),
            { tone: "ok" },
          );
        } else {
          dispatch({ type: "invalidDrop", message: tNow("folder.errors.notFolderOrPicture") });
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
    const picked = await open({ directory: true, multiple: false, title: tNow("common.dialog.chooseFolder") }).catch(() => null);
    if (typeof picked === "string") await takePath(picked);
  }, [takePath]);

  const pickPhoto = useCallback(async () => {
    if (!isTauri()) return takePath("/Users/you/Pictures/Lighthouse at dusk.jpg");
    const picked = await open({
      multiple: false,
      title: tNow("common.dialog.choosePicture"),
      filters: [{ name: tNow("common.dialog.pictures"), extensions: IMAGE_EXTENSIONS }],
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
            .treeBytes(ask.skin.id, ask.folderPath || undefined)
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
   * Whether another run is going, which a run over this folder's tree waits for (one at a time):
   * the folder panel says so, and the sentence comes back for anywhere else to say it.
   */
  const busySaid = useRef<string | null>(null);
  const refusedFor = useCallback((): string | null => {
    const going = treeRuns.now();
    if (!going?.running) return null;
    const message = tNow("folder.errors.busy", { folder: clip(going.name) });
    busySaid.current = message;
    dispatch({ type: "treeRefused", message });
    return message;
  }, []);
  // Once that run has ended, what it said is no longer so.
  useEffect(() => {
    if (run?.running || busySaid.current === null) return;
    if (latestState.current.error === busySaid.current) dispatch({ type: "clearError" });
    busySaid.current = null;
  }, [run?.running]);

  /**
   * Asks before a run over the tree that's big enough to ask about: an apply over more than a
   * handful of folders, in the words for a big one past `BIG_RUN` or while they're still being
   * counted, and every run that takes custom icons off. Resolves to the answer.
   */
  const confirmTree = useCallback(
    async (s: State, kind: "apply" | "remove", skin: Skin | null): Promise<boolean> => {
      const inside = insideCount(s);
      const counted = insideCounted(s);
      if (kind === "apply" && counted && inside + 1 <= CONFIRM_ABOVE) return true;
      return askTree({ kind, folderName: s.folder?.name ?? "", folderPath: s.folder?.path ?? "", inside, counted, chosen: s.chosen !== null, skin });
    },
    [askTree],
  );

  /**
   * Starts a run over the chosen folder's tree in the background: `skinId` on the folder and the
   * folders inside it the choice takes, or without one their custom icons off. It goes on whatever
   * the window shows, and the folder panel follows it while its folder is on show. Resolves to the
   * run as it starts, or to why it couldn't.
   */
  const startTree = useCallback(async (s: State, skinId: string | null): Promise<TreeRun | { refused: string }> => {
    const folder = s.folder;
    if (!folder) return { refused: tNow("folder.errors.chooseFirst") };
    // The first run that could end with nobody looking asks whether FolderSkin may say so.
    void api.askToNotify().catch(() => false);
    const choice = treeChoice(s);
    const chosen = choice && forRun(choice);
    // The folder itself and the ones inside it the count has found, once it has found them all.
    const expected = insideCounted(s) ? insideCount(s) + 1 : null;
    try {
      const event = skinId ? await api.startTreeApply(folder.path, skinId, chosen) : await api.startTreeRevert(folder.path, chosen, true);
      if (event.run && expected !== null) treeRuns.expect(event.run.id, expected);
      treeRuns.take(event);
      if (!event.run) return { refused: tNow("common.errors.somethingWrong") };
      dispatch({ type: "treeRunning", path: folder.path, run: runInfo(event.run) });
      return event.run;
    } catch (e) {
      const reason = errorMessage(e);
      const error = skinId ? tNow("folder.errors.apply", { reason }) : tNow("folder.errors.revertTree", { reason });
      dispatch({ type: "treeRefused", message: error });
      return { refused: error };
    }
  }, []);

  // The folder panel follows the run over its folder's tree: going, it shows how far the run has
  // got, and ended, how it went. The run goes on when another folder is picked, and picking its
  // folder again, or clicking it in the sidebar, brings it back to the panel.
  useEffect(() => {
    const s = latestState.current;
    if (!run || !s.folder || run.folder !== s.folder.path) return;
    const busy = s.phase === "applying" || s.phase === "reverting";
    if (run.running) {
      if (!(busy && s.runId === run.id)) dispatch({ type: "treeRunning", path: s.folder.path, run: runInfo(run) });
      return;
    }
    if (!busy || s.runId !== run.id) return;
    const reason = run.error === null ? null : explain(run.error);
    const error = reason === null ? null : run.kind === "apply" ? tNow("folder.errors.apply", { reason }) : tNow("folder.errors.revertTree", { reason });
    dispatch({ type: "treeEnded", run: runInfo(run), error });
    refreshFolderIcon(s.folder.path);
  }, [run, state.folder?.path, state.phase, state.runId, refreshFolderIcon]);

  // A run that started before the count of its folder was done: once the count is, it says how
  // many folders the run takes while the run's own walk is still finding them.
  const counted = insideCounted(state);
  useEffect(() => {
    const s = latestState.current;
    if (!run?.running || run.counted || !counted || s.runId !== run.id || s.folder?.path !== run.folder) return;
    if (treeRuns.expected(run.id) === null) treeRuns.expect(run.id, insideCount(s) + 1);
  }, [run, counted]);

  // A summary put away, or a run that has gone (replaced, or put away from the sidebar), leaves
  // the folder panel as it would be without one.
  useEffect(() => {
    if (state.runId !== null && (!run || run.id !== state.runId)) dispatch({ type: "runDismissed" });
  }, [run, state.runId]);

  // A run that ends while the window is behind others says so as a system notification.
  useEffect(
    () =>
      treeRuns.onEnded((ended) => {
        const notice = runNotice(ended, latestSkins.current.find((s) => s.id === ended.skin_id)?.name ?? null);
        if (!notice) return;
        void api
          .windowFocused()
          .then((focused) => (focused ? undefined : api.notify(notice.title, notice.body)))
          .catch(() => {});
      }),
    [],
  );

  const apply = useCallback(async () => {
    const s = latestState.current;
    const { folder, skinId } = s;
    if (!folder || !skinId) return;
    if (isTree(s)) {
      if (refusedFor()) return;
      const skin = latestSkins.current.find((k) => k.id === skinId) ?? null;
      if (!(await confirmTree(s, "apply", skin))) return;
      await startTree(latestState.current, skinId);
      return;
    }
    dispatch({ type: "applyStarted" });
    try {
      await api.applySkin(folder.path, skinId);
      dispatch({ type: "applySucceeded" });
      refreshFolderIcon(folder.path);
    } catch (e) {
      dispatch({ type: "applyFailed", message: tNow("folder.errors.apply", { reason: errorMessage(e) }) });
    }
  }, [refusedFor, confirmTree, startTree, refreshFolderIcon]);

  // Where the user is, for runs that finish after they may have moved on.
  const viewNow = useRef(view);
  viewNow.current = view;
  const carryOnNow = useRef<() => void>(() => {});

  /**
   * An apply over the tree, said in a toast for when the folder panel that sums it up isn't in
   * view: a stopped run offers to carry on, one with failures to show which.
   */
  const treeOutcome = useCallback((ended: TreeRun, folderName: string, skinName: string): ApplyOutcome => {
    const carry = canCarryOn(ended) && ended.changed > 0;
    return {
      ok: true,
      message: runToast(ended, folderName, skinName),
      tone: carry || ended.failed > 0 ? "info" : "ok",
      action: carry
        ? { label: tNow("folder.run.carryOn"), run: () => carryOnNow.current() }
        : ended.failed > 0
          ? { label: tNow("folder.run.seeWhich"), run: () => setView("yours") }
          : undefined,
    };
  }, []);

  /**
   * Carries on, tries again or undoes the latest run, as `action` does to it by id: the folder
   * panel follows it again when its folder is on show. Resolves to the run as it starts again.
   */
  const resumeRun = useCallback(
    async (action: (id: number) => Promise<TreeRunEvent>): Promise<TreeRun | null> => {
      const last = treeRuns.now();
      if (!last || last.running) return null;
      try {
        const event = await action(last.id);
        treeRuns.take(event);
        const at = latestState.current.folder?.path;
        if (event.run && at === event.run.folder) dispatch({ type: "treeRunning", path: at, run: runInfo(event.run) });
        return event.run;
      } catch (e) {
        toast(errorMessage(e), { tone: "danger" });
        return null;
      }
    },
    [toast],
  );

  /** Carries a stopped run on to the folders it didn't reach. */
  const carryOn = useCallback(async () => {
    const next = await resumeRun(api.carryOnTreeRun);
    // Carried on from the composer's toast: say how it ended there too.
    if (!next || viewNow.current !== "compose") return;
    const ended = await treeRuns.whenEnded(next.id);
    const said = treeOutcome(ended, ended.name, latestSkins.current.find((s) => s.id === ended.skin_id)?.name ?? tNow("folder.run.theSkin"));
    toast(said.message ?? "", { tone: said.tone, action: said.action });
  }, [resumeRun, treeOutcome, toast]);
  carryOnNow.current = () => void carryOn();

  /** Tries the folders the last run couldn't change once more. */
  const tryAgain = useCallback(() => void resumeRun(api.retryTreeRun), [resumeRun]);

  /** Takes off exactly what the last apply over a tree put on. */
  const undoRun = useCallback(() => void resumeRun(api.undoTreeRun), [resumeRun]);

  const stopRun = useCallback(() => {
    const going = treeRuns.now();
    if (going?.running) api.stopTreeRun(going.id).catch(() => {});
  }, []);

  /** Puts the last run's summary away, in the folder panel and the sidebar alike. */
  const dismissRun = useCallback(() => {
    const last = treeRuns.now();
    if (last && !last.running) api.dismissTreeRun(last.id).catch(() => {});
    dispatch({ type: "runDismissed" });
  }, []);

  // The composer's Save & apply: the same apply as the folder panel's, subfolders included when
  // they are, for a skin it just saved. Resolves to what happened, for the composer to say: a run
  // over the tree once it has ended, which it does in the background if the composer is left.
  const applyFromComposer = useCallback(
    async (skin: Skin): Promise<ApplyOutcome> => {
      const s = latestState.current;
      const { folder, phase } = s;
      if (!folder || phase === "applying" || phase === "reverting") return { ok: false };
      dispatch({ type: "skinSelected", skinId: skin.id });
      if (isTree(s)) {
        const busy = refusedFor();
        if (busy) return { ok: false, message: busy, tone: "danger" };
        if (!(await confirmTree(s, "apply", skin))) return { ok: false };
        const started = await startTree(latestState.current, skin.id);
        if ("refused" in started) return { ok: false, message: started.refused, tone: "danger" };
        const ended = await treeRuns.whenEnded(started.id);
        if (ended.error !== null) return { ok: false, message: tNow("folder.errors.apply", { reason: explain(ended.error) }), tone: "danger" };
        return treeOutcome(ended, folder.name, skin.name);
      }
      dispatch({ type: "applyStarted" });
      try {
        await api.applySkin(folder.path, skin.id);
        dispatch({ type: "applySucceeded" });
        refreshFolderIcon(folder.path);
        return { ok: true };
      } catch (e) {
        const message = tNow("folder.errors.apply", { reason: errorMessage(e) });
        dispatch({ type: "applyFailed", message });
        return { ok: false, message, tone: "danger" };
      }
    },
    [refusedFor, confirmTree, startTree, treeOutcome, refreshFolderIcon],
  );

  /** The run's folder back in the folder panel, from the sidebar, wherever the window is. */
  const showing = useRef<number | null>(null);
  const showRun = useCallback(() => {
    const last = treeRuns.now();
    if (!last) return;
    if (viewNow.current === "compose") setView("skins");
    setAiPanelHidden(false);
    const s = latestState.current;
    if (s.folder?.path === last.folder) {
      if (!last.running && s.runId !== last.id) dispatch({ type: "treeShown", path: last.folder, run: runInfo(last) });
      return;
    }
    // A run that's going is followed once its folder is in; an ended one shows its summary again.
    showing.current = last.running ? null : last.id;
    void takePath(last.folder);
  }, [takePath]);
  useEffect(() => {
    const id = showing.current;
    const last = treeRuns.now();
    if (id === null || !last || last.id !== id || state.folder?.path !== last.folder) return;
    showing.current = null;
    dispatch({ type: "treeShown", path: last.folder, run: runInfo(last) });
  }, [state.folder?.path]);

  // How many folders the choice made in "Choose subfolders" takes, as the count finds them.
  const choiceCount = useChoiceCount(state.folder?.path ?? null, state.chosen?.choice ?? null);
  useEffect(() => {
    const path = latestState.current.folder?.path;
    if (choiceCount && path) dispatch({ type: "choiceCounted", path, ...choiceCount });
  }, [choiceCount]);

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
    const s = latestState.current;
    const { folder, phase } = s;
    if (!folder) return;
    const last = treeRuns.now();
    const shown = last && last.id === s.runId && last.folder === folder.path && !last.running ? last : null;
    // Undoing an apply over the tree takes off exactly what it put on. A run stopped before it
    // changed anything leaves only the folder's own earlier apply, which goes as a single one does.
    if (phase === "applied" && shown?.kind === "apply" && shown.changed > 0) {
      undoRun();
      return;
    }
    // Anything else over the tree clears every icon in it, or in the folders chosen in it, which
    // asks first.
    if (isTree(s) && !(phase === "applied" && shown?.kind === "apply")) {
      if (refusedFor()) return;
      if (!(await confirmTree(s, "remove", null))) return;
      await startTree(latestState.current, null);
      return;
    }
    dispatch({ type: "revertStarted" });
    try {
      await api.revertSkin(folder.path);
      dispatch({ type: "revertSucceeded" });
      refreshFolderIcon(folder.path);
      toast(tNow("folder.toast.defaultBack", { name: clip(folder.name) }), { tone: "ok" });
    } catch (e) {
      dispatch({ type: "revertFailed", message: tNow("folder.errors.revert", { reason: errorMessage(e) }) });
    }
  }, [undoRun, refusedFor, confirmTree, startTree, refreshFolderIcon, toast]);

  const reveal = useCallback(() => {
    if (!state.folder) return;
    revealItemInDir(state.folder.path).catch((e) => toast(tNow("folder.errors.reveal", { reason: errorMessage(e) }), { tone: "danger" }));
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
          toast(tNow("library.toast.deleted", { name: clip(skin.name) }), { tone: "ok" });
        })
        .catch((e) => {
          setSkins((prev) => newestFirst([skin, ...prev.filter((s) => s.id !== skin.id)]));
          toast(tNow("library.errors.delete", { name: clip(skin.name), reason: errorMessage(e) }), { tone: "danger" });
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
          toast(tNow("library.errors.save", { name: clip(skin.name), reason: errorMessage(e) }), { tone: "danger" });
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
  // The run the folder panel follows: the latest, while its folder is on show and the panel took it up.
  const stageRun = run && state.folder && run.folder === state.folder.path && run.id === state.runId ? run : null;
  /** The skin the latest run puts on or takes off, as the library has it. */
  const runSkin = run?.skin_id ? (skins.find((s) => s.id === run.skin_id) ?? null) : null;
  // The question before a run over the tree says how many folders there are as the count goes on,
  // not only as it stood when the question was asked.
  const liveAsk = treeAsk && { ...treeAsk, inside: insideCount(state), counted: insideCounted(state) };
  // The icon of the folder on show, never one that arrived late for a folder picked before it.
  const stageIcon = folderIcon && folderIcon.path === state.folder?.path ? (folderIcon.url ?? defaultThumb) : undefined;
  const q = query.trim();
  const empty: Empty | null = loadError
    ? { icon: <FolderOpenIcon size={22} />, title: t("library.empty.loadFailed"), text: loadError }
    : visible.length > 0
      ? null
      : filtering
        ? {
            icon: <ListFilterIcon size={20} />,
            title: q ? t("library.empty.noMatchFiltered", { query: q }) : t("library.empty.noneFiltered"),
            text: t("library.empty.filteredText"),
            action: (
              <div className="empty-actions">
                {q && activeTag && (
                  <button type="button" className="btn btn-secondary" onClick={() => setTag("")}>
                    <SearchIcon size={15} />
                    {t("library.empty.searchAll")}
                  </button>
                )}
                <button type="button" className="btn btn-secondary" onClick={() => setFilters(NO_FILTERS)}>
                  {t("library.empty.clearFilters")}
                </button>
                {q && (
                  <button type="button" className="btn btn-secondary" onClick={() => setQuery("")}>
                    {t("library.empty.clearSearch")}
                  </button>
                )}
              </div>
            ),
          }
        : q
          ? {
              icon: <SearchIcon size={20} />,
              title: t("library.empty.noMatch", { query: q }),
              // "Look in All" only makes sense from a tag, and then the button does it.
              text: activeTag ? t("library.empty.noMatchText") : t("library.empty.noMatchTextAll"),
              action: activeTag ? (
                <div className="empty-actions">
                  <button type="button" className="btn btn-secondary" onClick={() => setTag("")}>
                    <SearchIcon size={15} />
                    {t("library.empty.searchAll")}
                  </button>
                  <button type="button" className="btn btn-secondary" onClick={() => setQuery("")}>
                    {t("library.empty.clearSearch")}
                  </button>
                </div>
              ) : (
                <button type="button" className="btn btn-secondary" onClick={() => setQuery("")}>
                  {t("library.empty.clearSearch")}
                </button>
              ),
            }
          : view === "faves"
            ? { icon: <StarIcon size={20} />, title: t("library.empty.noFaves"), text: t("library.empty.noFavesText") }
            : view === "skins" && skins.length === 0
              ? {
                  icon: <FolderOpenIcon size={22} />,
                  title: t("library.empty.noSkins"),
                  text: t("library.empty.noSkinsText"),
                  action: (
                    <div className="empty-actions">
                      <button type="button" className="btn btn-primary" onClick={() => setView("community")}>
                        {t("library.empty.browsePacks")}
                      </button>
                      <button type="button" className="btn btn-secondary" onClick={pickPhoto}>
                        {t("sidebar.addPhoto")}
                      </button>
                    </div>
                  ),
                }
              : null;

  const library = view === "skins" || view === "yours" || view === "faves";
  // The skin being tried is put down with Escape in the library, or a click on the grid's empty
  // space, as a selection is: the folder shows as it is again, or the empty folder with the Mac |
  // Windows switch under it. Not while typing, or while a menu or dialog has the key.
  const skinId = state.skinId;
  const putDown = useCallback(() => dispatch({ type: "skinCleared" }), []);
  useEffect(() => {
    if (!library || !skinId) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented || menu || aboutOpen || document.querySelector(".modal-backdrop, .cmp-pop, .filter-pop")) return;
      if (e.target instanceof Element && e.target.closest("input, textarea, [contenteditable='true']")) return;
      putDown();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [library, skinId, menu, aboutOpen, putDown]);
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
        label={t("library.resize.sidebar")}
        className="is-left"
        width={cols.left}
        min={layout.rail ? RAIL : LEFT.min}
        max={LEFT.max}
        grows="right"
        onWidth={resizeSidebar}
        onStep={(by) => stepSidebarEdge(cols.left, by)}
        onReset={() => setLayout((l) => ({ ...l, rail: false, left: DEFAULT_LAYOUT.left }))}
      />
      {rightShown && (
      <IslandResizer
        label={composing ? t("library.resize.composer") : t("library.resize.folder")}
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
        dock={
          run && (
            <RunDock
              run={run}
              skin={runSkin}
              folderIcon={defaultThumb}
              rail={layout.rail}
              onShow={showRun}
              onStop={stopRun}
              onCarryOn={() => void carryOn()}
              onRetry={tryAgain}
              onUndo={undoRun}
              onDismiss={dismissRun}
            />
          )
        }
      />
      <AboutMenu
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
        aria-label={t("library.label")}
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
              placeholder={activeTagName ? t("library.toolbar.searchIn", { tag: activeTagName }) : undefined}
              searchLabel={activeTagName ? t("library.toolbar.searchInLabel", { tag: activeTagName }) : undefined}
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
            <div
              className="gallery-scroll"
              aria-busy={redrawing ? true : undefined}
              onClick={(e) => {
                if (skinId && e.target instanceof Element && !e.target.closest(".tile, button, a, input")) putDown();
              }}
            >
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
            {redrawing && (
              <div className="gallery-redraw" role="status">
                <LoaderIcon size={15} />
                <span>{t(`folder.look.redrawing.${redrawing}`)}</span>
              </div>
            )}
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
          chosen={state.chosen}
          onIncludeSubfolders={(on) => dispatch({ type: "includeSubfolders", on })}
          progress={stageRun?.running ? treeProgress(stageRun, treeRuns.expected(stageRun.id)) : null}
          stopping={stageRun?.stopping ?? false}
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
        run={stageRun}
        runSkinName={runSkin?.name ?? null}
        onIncludeSubfolders={(on) => dispatch({ type: "includeSubfolders", on })}
        onChooseSubfolders={(chosen) => {
          const path = latestState.current.folder?.path;
          if (path) dispatch({ type: "subfoldersChosen", path, chosen });
        }}
        onStop={stopRun}
        onCarryOn={() => void carryOn()}
        onTryAgain={tryAgain}
        onDismissRun={dismissRun}
        pickHint={aiView ? t("folder.stage.pickHintChat") : undefined}
        onLook={chooseFolderLook}
        onPutDown={putDown}
      />
      </div>
      )}

      {confirmingDelete && (
        <Confirm
          title={t("library.delete.title", { name: clip(confirmingDelete.name) })}
          text={t("library.delete.text")}
          image={confirmingDelete.thumbnail}
          action={t("library.delete.action")}
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
      {liveAsk && (
        <Confirm
          title={askTitle(liveAsk)}
          text={askText(liveAsk)}
          image={liveAsk.kind === "apply" ? liveAsk.skin?.thumbnail : undefined}
          action={liveAsk.kind === "apply" ? applyLabel(liveAsk.inside, liveAsk.counted) : t("folder.ask.removeAction")}
          tone={liveAsk.kind === "apply" ? "primary" : "danger"}
          onCancel={() => answerTree(false)}
          onConfirm={() => answerTree(true)}
        />
      )}
      {sharing && <SharePack yours={yours} only={sharing.only} os={platform.os} onClose={closeSharing} />}
      {settingsTab && (
        <Settings
          tab={settingsTab}
          folderPicture={defaultThumb}
          themePref={themePref}
          onThemePref={setThemePref}
          rail={layout.rail}
          onRail={(on) => on !== layout.rail && toggleRail()}
          os={platform.os}
          savedCount={skins.filter((s) => s.custom).length}
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
