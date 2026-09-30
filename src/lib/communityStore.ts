/**
 * What the Community view is showing, kept outside the view so leaving it and coming back finds
 * everything as it was: the tab, the words typed, the tag, the order, the results, where each list
 * was scrolled to and a pack still being added. Nothing is fetched again on the way back.
 *
 * Two lists share the search box: the packs, narrowed by a tag, and the official collection, the
 * skins FolderSkin publishes on their own. Only the one on show is searched as the words change;
 * the other is searched when its tab is picked, unless what it shows already answers them.
 *
 * Searching: typing waits a moment (`SEARCH_DELAY_MS`) for the next key; a tag or an order
 * searches at once. Every search is numbered and only the newest one's answer is shown, so an
 * answer that comes back late can never put an old list over a newer one. The first page comes
 * with the answer; the others are asked for as their places scroll into view.
 *
 * Coming back, the packs shown are marked again against the library, which may have changed
 * meanwhile; that is a question for this computer, not a search.
 */
import { useSyncExternalStore } from "react";
import type { ToastTone } from "../hooks/useToasts";
import { clip } from "./names";
import { tagLabel } from "./tags";
import { t } from "../i18n";
import {
  api,
  errorMessage,
  type CollectionPage,
  type CollectionSkin,
  type CollectionSort,
  type CommunityPack,
  type CommunitySort,
  type InstallLink,
  type PackProgress,
  type Skin,
  type SkinHit,
  type UseFrom,
} from "./tauri";

/** Packs a page holds. */
export const PAGE = 60;
/** Official skins a page holds. */
export const SKIN_PAGE = 100;
/** How long typing waits for the next key before it searches. */
export const SEARCH_DELAY_MS = 100;

/** How the packs are shown: cards with bigger folders, or rows with their details. */
export type PackView = "gallery" | "list";

/** What is being done to a pack. */
export type PackTask = "add" | "update" | "remove";

/** Which list is on show: the packs, or the official collection's skins. */
export type CommunityTab = "packs" | "official";

/** One search's answer, as far as it has been paged in. */
export type Shown = {
  /** What it answers. */
  q: string;
  tag: string;
  sort: CommunitySort;
  /** Goes up with every new answer, so a page asked for an older one is dropped. */
  id: number;
  total: number;
  all: number;
  /** Every place in the list; a place whose page hasn't come yet is empty. */
  packs: (CommunityPack | undefined)[];
  skins: SkinHit[];
  /** Official skins the words match, which the strip of skins lists first. */
  official: CollectionSkin[];
  hitPacks: CommunityPack[];
  facets: { tag: string; count: number }[];
  /** Why these are the packs from the last visit ("you're offline"); null when they are current. */
  lastVisit: string | null;
  /** The catalog that answered. */
  generation: string;
};

/** One search of the official collection, as far as it has been paged in. */
export type ShownCollection = {
  q: string;
  sort: CollectionSort;
  /** Goes up with every new answer, so a page asked for an older one is dropped. */
  id: number;
  total: number;
  /** Every place in the list; a place whose page hasn't come yet is empty. */
  skins: (CollectionSkin | undefined)[];
};

export type CommunityState = {
  tab: CommunityTab;
  /** What is in the search box, as typed. */
  query: string;
  tag: string;
  sort: CommunitySort;
  view: PackView;
  /** The answer on screen; null until the first comes. */
  shown: Shown | null;
  /** A search for what is asked now hasn't answered yet. */
  searching: boolean;
  /** Why the last search failed, when it did. */
  error: string | null;
  refreshing: boolean;
  /** The pack being added, updated or removed, which of those, and how far it has got. */
  busy: string | null;
  task: PackTask | null;
  progress: PackProgress | null;
  /** The pack open in the viewer, and the skin to show it at. */
  viewing: { pack: CommunityPack; focus: number | null } | null;
  /** Where the list was scrolled to. */
  scrollTop: number;
  /** The official collection's order, its answer on screen (null until the first comes), whether
   *  a search of it hasn't answered yet, why the last one failed, and where its list was scrolled to. */
  collectionSort: CollectionSort;
  collection: ShownCollection | null;
  collectionSearching: boolean;
  collectionError: string | null;
  collectionScroll: number;
  /** How many official skins there are, for their tab; null until the app has said. */
  collectionSize: number | null;
  /** The licence every official skin comes under, such as "MIT"; empty until the app has said. */
  collectionLicense: string;
  /** The official skin open in the skin viewer. */
  viewingSkin: CollectionSkin | null;
  /** The skin being taken on its own ("Use"), by its picture's SHA-256. */
  using: string | null;
};

type Toast = (text: string, opts?: { tone?: ToastTone; action?: { label: string; run: () => void } }) => void;

/** What the app does when a pack comes or goes; the view hands over its latest each time it draws. */
export type CommunityHandlers = {
  onAdded: (skins: Skin[]) => void;
  onRemoved: (skinIds: string[]) => void;
  /** A skin taken on its own is in the library: the app picks it, where Apply is. */
  onUsed: (skin: Skin) => void;
  onShowTag: (tag: string) => void;
  toast: Toast;
};

const INITIAL: CommunityState = {
  tab: "packs",
  query: "",
  tag: "",
  sort: "best",
  view: "gallery",
  shown: null,
  searching: false,
  error: null,
  refreshing: false,
  busy: null,
  task: null,
  progress: null,
  viewing: null,
  scrollTop: 0,
  collectionSort: "newest",
  collection: null,
  collectionSearching: false,
  collectionError: null,
  collectionScroll: 0,
  collectionSize: null,
  collectionLicense: "",
  viewingSkin: null,
  using: null,
};

export class CommunityStore {
  private state: CommunityState = INITIAL;
  private listeners = new Set<() => void>();
  /** The newest search asked for. */
  private asked = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;
  /** Pages asked for, for the answer on screen. */
  private pages = new Set<number>();
  /** The same two for the official collection, and the answers it has shown, which its pages go by. */
  private askedSkins = 0;
  private skinPages = new Set<number>();
  private shownSkins = 0;
  /** Counting the official skins, while it is under way: asked once, however many want it. */
  private counting: Promise<void> | null = null;
  private handlers: CommunityHandlers | null = null;
  /** The pack `busy` names, for saying which one to wait for. */
  private working: CommunityPack | null = null;
  /** Goes up with every pack marked, so an older answer about the library isn't laid over it. */
  private marked = 0;
  /** What a folderskin://install link asked for, until there are handlers to act on it with. */
  private linked: InstallLink | null = null;

  constructor(private readonly delay = SEARCH_DELAY_MS) {}

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  get = () => this.state;

  private set(patch: Partial<CommunityState>) {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }

  bind(handlers: CommunityHandlers) {
    this.handlers = handlers;
    if (this.linked) void this.installLinked();
  }

  /**
   * A folderskin://install link, from an Install button on folderskin.app: the pack it names,
   * opened and added the way its Add button adds it, with the same progress and toasts. A pack in
   * the library already is opened and said to be there. From a Use in FolderSkin button, the link
   * names one skin instead: it is opened (an official one on its own, a pack's in its pack, at that
   * skin) and used the way its Use button uses it. The first time, it waits for the view's
   * handlers: a link can come before Community has ever been open.
   */
  install(link: InstallLink) {
    this.linked = link;
    if (this.handlers) void this.installLinked();
  }

  private async installLinked() {
    const link = this.linked;
    this.linked = null;
    if (link?.skin) return this.useLinked(link.skin, link.pack);
    const id = link?.pack;
    if (!id) return;
    let pack: CommunityPack | null;
    try {
      pack = await api.communityPack(id);
    } catch (e) {
      this.handlers?.toast(t("community.toast.addLinkFailed", { id, reason: errorMessage(e) }), { tone: "danger" });
      return;
    }
    if (!pack) {
      this.handlers?.toast(t("community.toast.noSuchPack", { id }), { tone: "danger" });
      return;
    }
    this.open(pack);
    if (pack.added) {
      const said = pack.update ? t("community.toast.alreadyNewer", { name: clip(pack.name) }) : t("community.toast.already", { name: clip(pack.name) });
      this.handlers?.toast(said, { tone: "ok", action: this.show(pack) });
      return;
    }
    await this.add(pack);
  }

  /** The skin a link names by its picture's SHA-256, in pack `packId` or, with none, official:
   *  opened where it is listed, and used. */
  private async useLinked(sha256: string, packId: string | null) {
    const toast = (text: string) => this.handlers?.toast(text, { tone: "danger" });
    try {
      if (!packId) {
        const skin = await api.communityCollectionSkin(sha256);
        if (!skin) return toast(t("community.toast.noSuchSkin"));
        this.showOfficial();
        this.openSkin(skin);
        return await this.use({ kind: "collection" }, sha256, skin.name);
      }
      const pack = await api.communityPack(packId);
      if (!pack) return toast(t("community.toast.noSuchSkin"));
      const skins = await api.packSkins(pack.id, pack.hash);
      const at = skins.findIndex((s) => s.sha256 === sha256);
      // Not in it any more: the pack is still worth a look.
      this.open(pack, at < 0 ? null : at);
      if (at < 0) return toast(t("community.toast.noSuchSkin"));
      await this.use({ kind: "pack", id: pack.id, hash: pack.hash }, sha256, skins[at].name);
    } catch (e) {
      toast(t("community.toast.useLinkFailed", { reason: errorMessage(e) }));
    }
  }

  /** Searches the first time the view opens; after that the answer is already here, and only
   *  whether its packs are in the library is asked again. The official skins are counted once,
   *  for their tab. */
  start() {
    if (!this.state.shown && !this.state.searching) void this.search();
    else if (this.state.shown) void this.remark();
    if (this.state.tab === "official" && !this.state.collection && !this.state.collectionSearching) void this.searchCollection();
    else if (this.state.collectionSize === null) void this.countCollection();
  }

  /** Searches ahead of the view's first visit (src/lib/warmUp.ts), when nothing is shown yet. */
  async warm(): Promise<void> {
    await Promise.all([
      !this.state.shown && !this.state.searching ? this.search() : undefined,
      this.state.collectionSize === null ? this.countCollection() : undefined,
    ]);
  }

  setQuery(query: string) {
    this.set({ query });
    clearTimeout(this.timer);
    this.timer = setTimeout(() => void (this.state.tab === "official" ? this.searchCollection() : this.search()), this.delay);
  }

  /** Shows the packs in tag `tag` ("" for all of them). */
  setTag(tag: string) {
    const back = this.state.tab === "official";
    this.set({ tag, tab: "packs" });
    // Back from the official skins to the packs they left, as they were: nothing to search again.
    if (back && this.answers(this.state.shown)) return;
    void this.search();
  }

  /** Shows the official skins, searched for the words in the box if they aren't already. */
  showOfficial() {
    if (this.state.tab === "official") return;
    this.set({ tab: "official" });
    if (!this.answersCollection(this.state.collection)) void this.searchCollection();
  }

  setSort(sort: CommunitySort) {
    this.set({ sort });
    void this.search();
  }

  setCollectionSort(sort: CollectionSort) {
    this.set({ collectionSort: sort });
    void this.searchCollection();
  }

  setView(view: PackView) {
    this.set({ view, scrollTop: 0 });
  }

  /** Remembers where the list on show is scrolled to. */
  keepScroll(scrollTop: number) {
    // Not worth telling anyone: it is only read when the view is drawn again.
    this.state = this.state.tab === "official" ? { ...this.state, collectionScroll: scrollTop } : { ...this.state, scrollTop };
  }

  /** Whether `shown` is the answer to what is asked of the packs now. */
  private answers(shown: Shown | null): boolean {
    const { query, tag, sort, searching } = this.state;
    return !searching && shown !== null && shown.q === query.trim() && shown.tag === tag && shown.sort === sort;
  }

  /** Whether `shown` is the answer to what is asked of the official skins now. */
  private answersCollection(shown: ShownCollection | null): boolean {
    const { query, collectionSort, collectionSearching } = this.state;
    return !collectionSearching && shown !== null && shown.q === query.trim() && shown.sort === collectionSort;
  }

  /** Searches for what is asked now. Resolves once it has answered, or been overtaken. The
   *  list starts at the top again unless `keepScroll`. */
  async search({ keepScroll = false } = {}): Promise<void> {
    clearTimeout(this.timer);
    const n = ++this.asked;
    const { query, tag, sort } = this.state;
    const q = query.trim();
    this.set({ searching: true });
    try {
      const r = await api.communitySearch({ q, tag, sort, offset: 0, limit: PAGE });
      if (n !== this.asked) return;
      const packs: (CommunityPack | undefined)[] = new Array(r.total);
      r.packs.forEach((p, i) => (packs[i] = p));
      this.pages = new Set([0]);
      const id = (this.state.shown?.id ?? 0) + 1;
      this.set({
        shown: {
          q,
          tag,
          sort,
          id,
          total: r.total,
          all: r.all,
          packs,
          skins: r.skins,
          official: r.collection ?? [],
          hitPacks: r.hit_packs,
          facets: r.facets,
          lastVisit: r.last_visit,
          generation: r.generation,
        },
        searching: false,
        error: null,
        scrollTop: keepScroll ? this.state.scrollTop : 0,
      });
    } catch (e) {
      if (n === this.asked) this.set({ searching: false, error: errorMessage(e) });
    }
  }

  /** Asks for the page holding place `index` of the list on screen, once. */
  need(index: number) {
    const shown = this.state.shown;
    if (!shown || index < 0 || index >= shown.total) return;
    const page = Math.floor(index / PAGE);
    if (this.pages.has(page)) return;
    this.pages.add(page);
    const { q, tag, sort, id } = shown;
    api
      .communitySearch({ q, tag, sort, offset: page * PAGE, limit: PAGE })
      .then((r) => {
        const now = this.state.shown;
        if (!now || now.id !== id) return;
        // A newer catalog came in under this list (Refresh, or back online): its pages wouldn't
        // line up with the rest, so the list is asked for again where it is.
        if (r.generation !== now.generation) {
          void this.search({ keepScroll: true });
          return;
        }
        const packs = now.packs.slice();
        r.packs.forEach((p, i) => (packs[page * PAGE + i] = p));
        this.set({ shown: { ...now, packs, lastVisit: r.last_visit } });
      })
      .catch(() => {
        // Asked again when that place is drawn again.
        this.pages.delete(page);
      });
  }

  /** Searches the official skins for what is asked now, as `search` does the packs. */
  async searchCollection(): Promise<void> {
    clearTimeout(this.timer);
    const n = ++this.askedSkins;
    const { query, collectionSort: sort } = this.state;
    const q = query.trim();
    this.set({ collectionSearching: true });
    try {
      const r = await api.communityCollection(q, sort, 0, SKIN_PAGE);
      if (n !== this.askedSkins) return;
      this.showCollection(r, q, sort);
      this.set({ collectionSearching: false, collectionError: null });
    } catch (e) {
      if (n === this.askedSkins) this.set({ collectionSearching: false, collectionError: errorMessage(e) });
    }
  }

  /** Counts the official skins for their tab, keeping the first page of them for when it's picked. */
  private countCollection(): Promise<void> {
    this.counting ??= (async () => {
      const n = this.askedSkins;
      try {
        const r = await api.communityCollection("", "newest", 0, SKIN_PAGE);
        this.set({ collectionSize: r.total, collectionLicense: r.license });
        // Nobody has searched them meanwhile: this is the answer to nothing typed, newest first.
        if (n === this.askedSkins && !this.state.collection) this.showCollection(r, "", "newest");
      } catch {
        // Counted again the next time the view opens.
      } finally {
        this.counting = null;
      }
    })();
    return this.counting;
  }

  /** Shows answer `r` for `q` in order `sort`, at the top of the list. */
  private showCollection(r: CollectionPage, q: string, sort: CollectionSort) {
    const skins: (CollectionSkin | undefined)[] = new Array(r.total);
    r.items.forEach((s, i) => (skins[i] = s));
    this.skinPages = new Set([0]);
    const id = ++this.shownSkins;
    this.set({
      collection: { q, sort, id, total: r.total, skins },
      // Every official skin, when nothing narrows them: the tab says so.
      ...(q === "" ? { collectionSize: r.total } : {}),
      collectionLicense: r.license,
      collectionScroll: 0,
    });
  }

  /** Asks for the page holding place `index` of the official skins on screen, once. */
  needSkin(index: number) {
    const shown = this.state.collection;
    if (!shown || index < 0 || index >= shown.total) return;
    const page = Math.floor(index / SKIN_PAGE);
    if (this.skinPages.has(page)) return;
    this.skinPages.add(page);
    const { q, sort, id } = shown;
    api
      .communityCollection(q, sort, page * SKIN_PAGE, SKIN_PAGE)
      .then((r) => {
        const now = this.state.collection;
        if (!now || now.id !== id) return;
        const skins = now.skins.slice();
        r.items.forEach((s, i) => (skins[page * SKIN_PAGE + i] = s));
        this.set({ collection: { ...now, skins } });
      })
      .catch(() => {
        // Asked again when that place is drawn again.
        this.skinPages.delete(page);
      });
  }

  open(pack: CommunityPack, focus: number | null = null) {
    this.set({ viewing: { pack, focus } });
  }

  close() {
    this.set({ viewing: null });
  }

  openSkin(skin: CollectionSkin) {
    this.set({ viewingSkin: skin });
  }

  closeSkin() {
    this.set({ viewingSkin: null });
  }

  /**
   * "Use": one skin, from the official collection or a pack, saved into the library without the
   * rest of its pack (which stays not added) and handed to the app, which picks it where Apply is.
   * The viewers close behind it, so coming back finds the list. One at a time.
   */
  async use(from: UseFrom, sha256: string, name: string) {
    if (this.state.using) return;
    this.set({ using: sha256 });
    try {
      const skin = await api.useCommunitySkin(from, sha256);
      this.set({ viewing: null, viewingSkin: null });
      this.handlers?.onUsed(skin);
    } catch (e) {
      this.handlers?.toast(t("community.toast.useFailed", { name: clip(name), reason: errorMessage(e) }), { tone: "danger" });
    } finally {
      this.set({ using: null });
    }
  }

  /** Marks pack `id` as in the library or not, everywhere it is shown. */
  mark(id: string | undefined, added: boolean) {
    if (!id) return;
    this.marked++;
    this.remarkWith((p) => (p.id === id ? { ...p, added, update: false } : p));
  }

  /** Marks every pack shown against what the library holds now. Left as it was if the app can't say. */
  async remark(): Promise<void> {
    const marked = this.marked;
    let installed: Record<string, string | null>;
    try {
      installed = await api.communityInstalled();
    } catch {
      return;
    }
    // A pack added or removed while the app answered: that answer is older than its mark.
    if (marked !== this.marked) return this.remark();
    this.remarkWith((p) => {
      // The pack being worked on is marked when that is done.
      if (p.id === this.state.busy) return p;
      const added = p.id in installed;
      // As the app decides it: a pack added before versions were kept is offered the update.
      const update = added && p.hash !== "" && installed[p.id] !== p.hash;
      return p.added === added && p.update === update ? p : { ...p, added, update };
    });
  }

  /** Passes every pack shown, in the list, the skins' packs and the viewer, through `change`. */
  private remarkWith(change: (p: CommunityPack) => CommunityPack) {
    const shown = this.state.shown;
    const viewing = this.state.viewing;
    this.set({
      shown: shown && { ...shown, packs: shown.packs.map((p) => p && change(p)), hitPacks: shown.hitPacks.map(change) },
      viewing: viewing && { ...viewing, pack: change(viewing.pack) },
    });
  }

  /** True, having said so, when another pack is being worked on: one at a time. */
  private waiting(): boolean {
    const working = this.working;
    if (!this.state.busy || !working) return false;
    const task = this.state.task === "add" ? "add" : this.state.task === "update" ? "update" : "remove";
    this.handlers?.toast(t(`community.toast.stillBusy.${task}`, { name: clip(working.name) }));
    return true;
  }

  async add(pack: CommunityPack) {
    if (this.waiting()) return;
    this.working = pack;
    this.set({ busy: pack.id, task: "add", progress: null });
    try {
      const skins = await api.addPack(pack.id, this.hear(pack.id));
      this.mark(pack.id, true);
      this.handlers?.onAdded(skins);
      this.handlers?.toast(t("community.toast.added", { count: skins.length, name: clip(pack.name) }), { tone: "ok", action: this.show(pack) });
    } catch (e) {
      this.handlers?.toast(t("community.toast.addFailed", { name: clip(pack.name), reason: errorMessage(e) }), { tone: "danger" });
    } finally {
      this.working = null;
      this.set({ busy: null, task: null, progress: null });
    }
  }

  async update(pack: CommunityPack) {
    if (this.waiting()) return;
    this.working = pack;
    this.set({ busy: pack.id, task: "update", progress: null });
    try {
      const { removed, skins } = await api.updatePack(pack.id, this.hear(pack.id));
      if (removed.length) this.handlers?.onRemoved(removed);
      this.handlers?.onAdded(skins);
      this.mark(pack.id, true);
      this.handlers?.toast(t("community.toast.updated", { name: clip(pack.name) }), { tone: "ok", action: this.show(pack) });
    } catch (e) {
      this.handlers?.toast(t("community.toast.updateFailed", { name: clip(pack.name), reason: errorMessage(e) }), { tone: "danger" });
    } finally {
      this.working = null;
      this.set({ busy: null, task: null, progress: null });
    }
  }

  async remove(pack: CommunityPack) {
    if (this.waiting()) return;
    this.working = pack;
    this.set({ busy: pack.id, task: "remove" });
    try {
      this.handlers?.onRemoved(await api.removePack(pack.id));
      this.mark(pack.id, false);
      this.handlers?.toast(t("community.toast.removed", { name: clip(pack.name) }), { tone: "ok" });
    } catch (e) {
      this.handlers?.toast(t("community.toast.removeFailed", { name: clip(pack.name), reason: errorMessage(e) }), { tone: "danger" });
    } finally {
      this.working = null;
      this.set({ busy: null, task: null });
    }
  }

  /** Passes on how far work on pack `id` has got, while it is still the pack being worked on. */
  private hear(id: string) {
    return (progress: PackProgress) => {
      if (this.state.busy === id) this.set({ progress });
    };
  }

  /** Asks for the packs again past every cache, then shows the list afresh, and the official
   *  skins too: on show they're searched again, and otherwise counted again and searched when picked. */
  async refresh() {
    if (this.state.refreshing) return;
    this.set({ refreshing: true });
    try {
      const { updates } = await api.communityRefresh();
      if (this.state.tab !== "official") this.set({ collection: null });
      await Promise.all([this.search(), this.state.tab === "official" ? this.searchCollection() : this.countCollection()]);
      this.handlers?.toast(
        updates ? t("community.toast.updates", { count: updates }) : t("community.toast.upToDate"),
        { tone: "ok" },
      );
    } catch (e) {
      this.handlers?.toast(t("community.toast.refreshFailed", { reason: errorMessage(e) }), { tone: "danger" });
    } finally {
      this.set({ refreshing: false });
    }
  }

  /** "Show" on a toast: the library, at the pack's first tag. */
  private show(pack: CommunityPack) {
    const tag = pack.tags[0];
    const handlers = this.handlers;
    return tag && handlers ? { label: t("community.toast.show"), run: () => handlers.onShowTag(tag) } : undefined;
  }
}

/** The one the Community view uses. */
export const community = new CommunityStore();

export function useCommunity(store: CommunityStore = community): CommunityState {
  return useSyncExternalStore(store.subscribe, store.get);
}

/** How far adding a pack has got, from 0 to 1: downloading is most of the wait, saving the rest. */
export function progressShare(p: PackProgress | null): number {
  if (!p || p.total === 0) return 0;
  const part = Math.min(p.done, p.total) / p.total;
  return p.stage === "download" ? part * 0.85 : 0.85 + part * 0.15;
}

/** "Downloading 3 of 16", "Saving 16 of 16", or `before` ("Adding", "Updating") until anything is heard. */
export function progressLabel(p: PackProgress | null, before?: string): string {
  if (!p) return before ?? t("community.progress.adding");
  const done = Math.min(p.done, p.total);
  return p.stage === "download" ? t("community.progress.downloading", { done, total: p.total }) : t("community.progress.saving", { done, total: p.total });
}

/**
 * Why the list is the one from the last visit, as a sentence: the Rust side says why as the start
 * of one in English ("you're offline", catalog.rs `LastVisit`), which is said again here.
 */
function lastVisitLine(why: string): string {
  if (why === "you're offline") return t("community.count.lastVisit.offline");
  if (why === "the newest list of packs didn't arrive") return t("community.count.lastVisit.behind");
  const host = /^(?<host>\S+) isn't answering$/.exec(why)?.groups?.host;
  if (host) return t("community.count.lastVisit.unanswered", { host });
  return t("community.count.lastVisit.other", { why: why.charAt(0).toUpperCase() + why.slice(1) });
}

/** What the list holds, in a few words: "10,000 packs", "1 pack matches “koi”", "23 packs match “koi”". */
export function countLine(shown: Shown | null, error: string | null): string {
  if (!shown) return "";
  if (error) return t("community.count.searchFailed", { reason: error });
  const count = shown.total;
  const tag = shown.tag ? tagLabel(shown.tag) : "";
  const line = shown.q
    ? tag
      ? t("community.count.matchingTagged", { count, query: shown.q, tag })
      : t("community.count.matching", { count, query: shown.q })
    : tag
      ? t("community.count.tagged", { count, tag })
      : t("community.count.packs", { count });
  return shown.lastVisit ? t("community.count.withLastVisit", { line, why: lastVisitLine(shown.lastVisit) }) : line;
}

/** What the official list holds, in a few words: "587 official skins", "12 official skins match “cat”". */
export function collectionCountLine(shown: ShownCollection | null, error: string | null): string {
  if (!shown) return "";
  if (error) return t("community.count.searchFailed", { reason: error });
  const count = shown.total;
  return shown.q ? t("community.count.officialMatching", { count, query: shown.q }) : t("community.count.official", { count });
}
