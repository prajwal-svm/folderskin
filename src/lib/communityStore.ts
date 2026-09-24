/**
 * What the Community view is showing, kept outside the view so leaving it and coming back finds
 * everything as it was: the words typed, the tag, the order, the results, where the list was
 * scrolled to and a pack still being added. Nothing is fetched again on the way back.
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
import { api, errorMessage, type CommunityPack, type CommunitySort, type PackProgress, type Skin, type SkinHit } from "./tauri";

/** Packs a page holds. */
export const PAGE = 60;
/** How long typing waits for the next key before it searches. */
export const SEARCH_DELAY_MS = 100;

/** How the packs are shown: cards with bigger folders, or rows with their details. */
export type PackView = "gallery" | "list";

/** What is being done to a pack. */
export type PackTask = "add" | "update" | "remove";

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
  hitPacks: CommunityPack[];
  facets: { tag: string; count: number }[];
  /** Why these are the packs from the last visit ("you're offline"); null when they are current. */
  lastVisit: string | null;
  /** The catalog that answered. */
  generation: string;
};

export type CommunityState = {
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
};

type Toast = (text: string, opts?: { tone?: ToastTone; action?: { label: string; run: () => void } }) => void;

/** What the app does when a pack comes or goes; the view hands over its latest each time it draws. */
export type CommunityHandlers = {
  onAdded: (skins: Skin[]) => void;
  onRemoved: (skinIds: string[]) => void;
  onShowTag: (tag: string) => void;
  toast: Toast;
};

const INITIAL: CommunityState = {
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
};

export class CommunityStore {
  private state: CommunityState = INITIAL;
  private listeners = new Set<() => void>();
  /** The newest search asked for. */
  private asked = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;
  /** Pages asked for, for the answer on screen. */
  private pages = new Set<number>();
  private handlers: CommunityHandlers | null = null;
  /** The pack `busy` names, for saying which one to wait for. */
  private working: CommunityPack | null = null;
  /** Goes up with every pack marked, so an older answer about the library isn't laid over it. */
  private marked = 0;

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
  }

  /** Searches the first time the view opens; after that the answer is already here, and only
   *  whether its packs are in the library is asked again. */
  start() {
    if (!this.state.shown && !this.state.searching) void this.search();
    else if (this.state.shown) void this.remark();
  }

  setQuery(query: string) {
    this.set({ query });
    clearTimeout(this.timer);
    this.timer = setTimeout(() => void this.search(), this.delay);
  }

  setTag(tag: string) {
    this.set({ tag });
    void this.search();
  }

  setSort(sort: CommunitySort) {
    this.set({ sort });
    void this.search();
  }

  setView(view: PackView) {
    this.set({ view, scrollTop: 0 });
  }

  keepScroll(scrollTop: number) {
    // Not worth telling anyone: it is only read when the view is drawn again.
    this.state = { ...this.state, scrollTop };
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

  open(pack: CommunityPack, focus: number | null = null) {
    this.set({ viewing: { pack, focus } });
  }

  close() {
    this.set({ viewing: null });
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
    const doing = this.state.task === "add" ? "added" : this.state.task === "update" ? "updated" : "removed";
    this.handlers?.toast(`${clip(working.name)} is still being ${doing}. Try again once it's done.`);
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
      this.handlers?.toast(`Added ${skins.length} skins from ${clip(pack.name)}`, { tone: "ok", action: this.show(pack) });
    } catch (e) {
      this.handlers?.toast(`Couldn't add ${clip(pack.name)}: ${errorMessage(e)}`, { tone: "danger" });
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
      this.handlers?.toast(`Updated ${clip(pack.name)}`, { tone: "ok", action: this.show(pack) });
    } catch (e) {
      this.handlers?.toast(`Couldn't update ${clip(pack.name)}: ${errorMessage(e)}`, { tone: "danger" });
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
      this.handlers?.toast(`Removed ${clip(pack.name)}`, { tone: "ok" });
    } catch (e) {
      this.handlers?.toast(`Couldn't remove ${clip(pack.name)}: ${errorMessage(e)}`, { tone: "danger" });
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

  /** Asks for the packs again past every cache, then shows the list afresh. */
  async refresh() {
    if (this.state.refreshing) return;
    this.set({ refreshing: true });
    try {
      const { updates } = await api.communityRefresh();
      await this.search();
      this.handlers?.toast(
        updates ? `${updates} of your packs ${updates === 1 ? "has" : "have"} an update` : "Community is up to date",
        { tone: "ok" },
      );
    } catch (e) {
      this.handlers?.toast(`Couldn't refresh: ${errorMessage(e)}`, { tone: "danger" });
    } finally {
      this.set({ refreshing: false });
    }
  }

  /** "Show" on a toast: the library, at the pack's first tag. */
  private show(pack: CommunityPack) {
    const tag = pack.tags[0];
    const handlers = this.handlers;
    return tag && handlers ? { label: "Show", run: () => handlers.onShowTag(tag) } : undefined;
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
export function progressLabel(p: PackProgress | null, before = "Adding"): string {
  if (!p) return before;
  return `${p.stage === "download" ? "Downloading" : "Saving"} ${Math.min(p.done, p.total)} of ${p.total}`;
}

const numbers = new Intl.NumberFormat("en-GB");

/** What the list holds, in a few words: "10,000 packs", "1 pack matches “koi”", "23 packs match “koi”". */
export function countLine(shown: Shown | null, error: string | null): string {
  if (!shown) return "";
  if (error) return `Couldn't search: ${error}`;
  const one = shown.total === 1;
  const packs = `${numbers.format(shown.total)} ${one ? "pack" : "packs"}`;
  const words = shown.q ? ` ${one ? "matches" : "match"} “${shown.q}”` : "";
  const tagged = shown.tag ? ` tagged ${tagLabel(shown.tag)}` : "";
  const lastVisit = shown.lastVisit ? `. ${shown.lastVisit.charAt(0).toUpperCase()}${shown.lastVisit.slice(1)}, so these are the packs from your last visit` : "";
  return `${packs}${words}${tagged}${lastVisit}`;
}
