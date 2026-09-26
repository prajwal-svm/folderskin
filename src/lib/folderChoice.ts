/**
 * Which of the folders inside a folder a run over its tree takes: the tree `subfolder_tree` reads
 * (src-tauri/src/tree.rs), with a tick for each folder. Pure, so it's tested without the app.
 *
 * A folder's box speaks for the folder and everything inside it. Ticking it ticks all of them,
 * clearing it clears all of them, and a folder with only some of them ticked shows a dash. The
 * folder the tree was read from always takes part, so it has no box of its own.
 *
 * Every folder keeps how many folders it holds and how many of those are ticked, and everything
 * inside a folder sits together in one stretch of the lists, so a tick fills one stretch and
 * updates the folders it's in, however big the tree: at most 5,000 folders (a run's limit), any
 * number of them in one folder, any number of levels deep.
 */

/** What `subfolder_tree` sends: every folder inside `root` a run would take, and where each one is. */
export type SubfolderTree = {
  /** The folder itself, as a run names it. */
  root: string;
  /** What goes between a folder's path and a name inside it: "/", or "\" on Windows. */
  separator: string;
  /** Every folder inside, nearest first, the way a run goes through them. */
  names: string[];
  /** The folder each one is in: 0 for `root`, `i + 1` for `names[i]`, always one before it. */
  parents: number[];
  /** True when there are more than a run can take, and `names` stops there. */
  more: boolean;
};

/** How a folder's box shows: ticked (it and everything inside it), clear (none of them), or a dash (some). */
export type Check = "on" | "off" | "mixed";

/** Folders by name as Finder lists them: "Photos 2" before "Photos 10", capitals or not. */
export const byName = (locale?: string) => new Intl.Collator(locale, { numeric: true });

/**
 * A tree of folders and which of them are ticked. Folders are numbered: 0 is the folder the tree
 * was read from, and `i + 1` is `names[i]`, so the numbers follow the order a run goes in.
 *
 * Inside, the ticks and counts are kept in depth-first order instead ("at", below): a folder,
 * then everything inside it, so all of a folder's insides are the `weight` places from its own.
 */
export class FolderChoice {
  /** The folder the tree was read from. */
  readonly root: string;
  readonly separator: string;
  /** Folders inside the root, the ones that can be ticked. */
  readonly total: number;

  private readonly names: string[];
  private readonly parent: Int32Array;
  private readonly level: Int32Array;
  /** Every folder's children, grouped by the folder they're in, in the walk's order: those of
   *  folder k are `kids[start[k]]` up to `kids[start[k + 1]]`. */
  private readonly start: Int32Array;
  private readonly kids: Int32Array;
  /** Where each folder is in depth-first order. */
  private readonly at: Int32Array;
  /** By depth-first place: the folder and everything inside it, as a count. */
  private readonly weight: Int32Array;
  /** By depth-first place: how many of those are ticked. */
  private readonly ticked: Int32Array;
  /** By depth-first place: whether the folder itself is ticked. */
  private readonly own: Uint8Array;
  /** Children in the order they're shown, per folder, sorted the first time they're asked for. */
  private readonly shown: (Int32Array | undefined)[];
  /** Where each folder is among the folders beside it, as shown. */
  private readonly place: Int32Array;
  private readonly compare: (a: string, b: string) => number;
  private paths: string[] | null = null;

  constructor(tree: SubfolderTree, collator: Intl.Collator = byName()) {
    const n = tree.names.length;
    if (tree.parents.length !== n) throw new Error("the tree has a different number of names and parents");
    const count = n + 1;
    this.root = tree.root;
    this.separator = tree.separator;
    this.total = n;
    this.names = tree.names;
    this.parent = new Int32Array(count);
    this.level = new Int32Array(count);
    this.parent[0] = -1;
    const kidCount = new Int32Array(count);
    for (let i = 1; i < count; i++) {
      const p = tree.parents[i - 1];
      if (!Number.isInteger(p) || p < 0 || p >= i) throw new Error(`${tree.names[i - 1]} is inside a folder that comes after it`);
      this.parent[i] = p;
      this.level[i] = this.level[p] + 1;
      kidCount[p] += 1;
    }
    // Children grouped by their folder, keeping the walk's order within each: a counting sort.
    this.start = new Int32Array(count + 1);
    for (let k = 0; k < count; k++) this.start[k + 1] = this.start[k] + kidCount[k];
    this.kids = new Int32Array(n);
    const next = this.start.slice(0, count);
    for (let i = 1; i < count; i++) this.kids[next[this.parent[i]]++] = i;
    // Depth-first places, from a stack rather than recursion: a branch can be thousands deep.
    this.at = new Int32Array(count);
    const stack = new Int32Array(count);
    let top = 0;
    let place = 0;
    stack[top++] = 0;
    while (top > 0) {
      const k = stack[--top];
      this.at[k] = place++;
      for (let j = this.start[k + 1] - 1; j >= this.start[k]; j--) stack[top++] = this.kids[j];
    }
    // Every folder comes after the one it's in, so one sweep back adds each folder into its parent.
    const weight = new Int32Array(count).fill(1);
    for (let i = count - 1; i > 0; i--) weight[this.parent[i]] += weight[i];
    this.weight = new Int32Array(count);
    for (let k = 0; k < count; k++) this.weight[this.at[k]] = weight[k];
    // Everything starts ticked, as "Include subfolders" takes every folder inside.
    this.own = new Uint8Array(count).fill(1);
    this.ticked = this.weight.slice();
    this.shown = new Array(count);
    this.place = new Int32Array(count);
    this.compare = collator.compare;
  }

  /** Ticked folders inside the root. */
  get chosen(): number {
    return this.ticked[0] - 1;
  }

  /** The folder's name. The root's is the last part of its path. */
  name(node: number): string {
    if (node > 0) return this.names[node - 1];
    const parts = this.root.split(this.separator).filter(Boolean);
    return parts[parts.length - 1] ?? this.root;
  }

  /** How far down it is: 1 for a folder directly inside the root. */
  depth(node: number): number {
    return this.level[node];
  }

  /** The folder it's in, or -1 for the root. */
  parentOf(node: number): number {
    return this.parent[node];
  }

  /** How many folders are directly inside it. */
  childCount(node: number): number {
    return this.start[node + 1] - this.start[node];
  }

  hasChildren(node: number): boolean {
    return this.start[node + 1] > this.start[node];
  }

  /** Every folder inside it, the folder itself not counted. */
  inside(node: number): number {
    return this.weight[this.at[node]] - 1;
  }

  /** How many folders inside it are ticked. */
  chosenInside(node: number): number {
    const i = this.at[node];
    return this.ticked[i] - this.own[i];
  }

  /** The folders directly inside it, in the order Finder lists them. Don't change the list. */
  children(node: number): Int32Array {
    let list = this.shown[node];
    if (!list) {
      list = this.kids.slice(this.start[node], this.start[node + 1]);
      const names = this.names;
      const compare = this.compare;
      // The same name spelled differently only in ways the language ignores still has one order.
      list.sort((a, b) => compare(names[a - 1], names[b - 1]) || (names[a - 1] < names[b - 1] ? -1 : names[a - 1] > names[b - 1] ? 1 : a - b));
      for (let i = 0; i < list.length; i++) this.place[list[i]] = i;
      this.shown[node] = list;
    }
    return list;
  }

  /** Where the folder is among the folders beside it, as `children` lists them. */
  indexOf(node: number): number {
    if (node <= 0) return 0;
    this.children(this.parent[node]);
    return this.place[node];
  }

  /** How its box shows. */
  check(node: number): Check {
    const i = this.at[node];
    const ticked = this.ticked[i];
    if (ticked === this.weight[i]) return "on";
    return ticked === 0 ? "off" : "mixed";
  }

  /** Whether the folder itself is ticked (a dash leaves it either way). The root always is. */
  isChosen(node: number): boolean {
    return this.own[this.at[node]] === 1;
  }

  /** Ticks or clears the folder and everything inside it. False when that changes nothing. */
  set(node: number, on: boolean): boolean {
    if (node <= 0 || node > this.total) return false;
    const from = this.at[node];
    const to = from + this.weight[from];
    const change = (on ? this.weight[from] : 0) - this.ticked[from];
    if (change === 0) return false;
    this.own.fill(on ? 1 : 0, from, to);
    if (on) this.ticked.set(this.weight.subarray(from, to), from);
    else this.ticked.fill(0, from, to);
    for (let a = this.parent[node]; a >= 0; a = this.parent[a]) this.ticked[this.at[a]] += change;
    return true;
  }

  /** What a click on its box does: a ticked folder is cleared, a clear one or a dash is ticked, with everything inside it. */
  toggle(node: number): boolean {
    return this.set(node, this.check(node) !== "on");
  }

  /** Ticks every folder inside the root, or clears them all. */
  setAll(on: boolean) {
    if (on) {
      this.own.fill(1);
      this.ticked.set(this.weight);
    } else {
      this.own.fill(0);
      this.ticked.fill(0);
      this.own[0] = 1;
      this.ticked[0] = 1;
    }
  }

  /** Ticks exactly the folders at `paths` inside the root, and clears the rest. */
  pick(paths: Iterable<string>) {
    const wanted = new Set(paths);
    const all = this.allPaths();
    const { own, ticked, at, parent } = this;
    for (let k = 1; k < all.length; k++) own[at[k]] = wanted.has(all[k]) ? 1 : 0;
    own[0] = 1;
    ticked.set(own);
    // Deepest first, so each folder's count is whole before it's added to its parent's.
    for (let k = all.length - 1; k > 0; k--) ticked[at[parent[k]]] += ticked[at[k]];
  }

  /** The folders from the first level down to `node`, the root not among them. */
  chain(node: number): number[] {
    const out: number[] = [];
    for (let k = node; k > 0; k = this.parent[k]) out.push(k);
    return out.reverse();
  }

  /** The folder's path, as a run names it. */
  path(node: number): string {
    return this.allPaths()[node];
  }

  /** The ticked folders inside the root, nearest first as a run goes through them: what `only` names besides the root. */
  chosenPaths(): string[] {
    const all = this.allPaths();
    const out: string[] = [];
    for (let k = 1; k < all.length; k++) if (this.own[this.at[k]]) out.push(all[k]);
    return out;
  }

  /** Every folder's path, made once: its folder's path, the separator and its name. */
  private allPaths(): string[] {
    if (!this.paths) {
      const paths = new Array<string>(this.total + 1);
      paths[0] = this.root;
      for (let k = 1; k < paths.length; k++) paths[k] = paths[this.parent[k]] + this.separator + this.names[k - 1];
      this.paths = paths;
    }
    return this.paths;
  }
}
