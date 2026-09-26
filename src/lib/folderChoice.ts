/**
 * Which of the folders inside a folder a run over its tree takes, as "Choose subfolders"
 * (components/SubfolderChooser.tsx) leaves them. Pure, so it's tested without the app.
 *
 * A choice is rules, not a list. Ticking or clearing a folder decides it and everything inside
 * it, unless a rule further down says otherwise, so a choice is as small as the ticks that made
 * it, however big the tree, and nothing needs the tree in memory. The run reads the same rules
 * (`ChoiceDto` in src-tauri/src/tree.rs). The folder the choice is in always takes part, so it
 * has no box of its own.
 *
 * The rules are kept so that each one says something the folder it's in doesn't: ticking a
 * folder drops every rule inside it, and a rule that agrees with the folder it's in isn't kept.
 * So a folder with a rule anywhere inside it has some of its folders ticked and some not, and
 * shows a dash, without anything being counted.
 */

import type { PathCount } from "./tree";

/** How a folder's box shows: ticked (it and everything inside it), clear (none of them), or a dash (some). */
export type Check = "on" | "off" | "mixed";

export type Choice = {
  /** The folder the choice is in, as a run names it (`subfolder_list`'s `path`). */
  root: string;
  /** What goes between a folder's path and a name inside it: "/", or "\" on Windows. */
  separator: string;
  /** Whether the folders inside are ticked where no rule says otherwise. */
  all: boolean;
  /** Folders ticked (true) or cleared (false) with everything inside them, by path. */
  rules: Record<string, boolean>;
};

/** Folders by name as Finder lists them: "Photos 2" before "Photos 10", capitals or not. */
export const byName = (locale?: string) => new Intl.Collator(locale, { numeric: true });

/** Every folder inside `root`, as "Include subfolders" takes them before anything is chosen. */
export const everything = (root: string, separator: string): Choice => ({ root, separator, all: true, rules: {} });

/** It takes every folder inside: nothing was cleared, or everything was ticked again. */
export const isEverything = (choice: Choice) => choice.all && Object.keys(choice.rules).length === 0;

/** It takes no folder inside, so the folder goes on its own. */
export const isNothing = (choice: Choice) => !choice.all && Object.keys(choice.rules).length === 0;

/** The path of the folder called `name` inside the folder at `parent`, with the `separator` the paths use. */
export const pathIn = ({ separator }: { separator: string }, parent: string, name: string) => parent + separator + name;

/** The folder that `path` is in. */
const parentOf = (choice: Choice, path: string) => path.slice(0, path.lastIndexOf(choice.separator));

/** Whether `path` is inside `folder`, however far down. */
const isInside = (choice: Choice, path: string, folder: string) => path.startsWith(folder + choice.separator);

const ruleAt = (choice: Choice, path: string): boolean | undefined => (Object.hasOwn(choice.rules, path) ? choice.rules[path] : undefined);

/** Whether the folder at `path` is ticked: its own rule, or the nearest one above it, or `all`. */
export function takes(choice: Choice, path: string): boolean {
  for (let at = path; at.length > choice.root.length && isInside(choice, at, choice.root); at = parentOf(choice, at)) {
    const rule = ruleAt(choice, at);
    if (rule !== undefined) return rule;
  }
  return choice.all;
}

/** How the box of the folder at `path` shows: as it's ticked, or a dash when a rule inside it says otherwise. */
export function check(choice: Choice, path: string): Check {
  for (const ruled of Object.keys(choice.rules)) if (isInside(choice, ruled, path)) return "mixed";
  return takes(choice, path) ? "on" : "off";
}

/** Ticks or clears the folder at `path` and everything inside it. */
export function set(choice: Choice, path: string, on: boolean): Choice {
  const rules: Record<string, boolean> = {};
  for (const [ruled, value] of Object.entries(choice.rules)) {
    if (ruled !== path && !isInside(choice, ruled, path)) rules[ruled] = value;
  }
  // A rule that says what the folder it's in says anyway isn't kept.
  if (takes({ ...choice, rules }, parentOf(choice, path)) !== on) rules[path] = on;
  return { ...choice, rules };
}

/** What a click on a folder's box does: a ticked folder is cleared, a clear one or a dash is ticked, with everything inside it. */
export function toggle(choice: Choice, path: string): Choice {
  return set(choice, path, check(choice, path) !== "on");
}

/** Ticks every folder inside the root, or clears them all. */
export const setAll = (choice: Choice, on: boolean): Choice => ({ ...choice, all: on, rules: {} });

/** The folders a rule is on, for asking how many folders each holds. */
export const ruledPaths = (choice: Choice) => Object.keys(choice.rules);

/** The choice as a run takes it: `ChoiceDto` in src-tauri/src/tree.rs. */
export const forRun = (choice: Choice) => ({ all: choice.all, rules: Object.entries(choice.rules).map(([path, on]) => ({ path, on })) });

/** How many folders inside the root a choice takes, and how many there are: final once `done`. */
export type ChoiceCount = { count: number; total: number; done: boolean };

/**
 * How many folders inside the root `choice` takes, from the count of everything inside the root
 * (`root`) and of each folder a rule is on (`counts`, in `ruledPaths` order).
 *
 * Every folder counts as `all` says, and each rule then changes its own folder and everything
 * inside it from what the folder it's in says to what the rule says, so a rule inside another
 * undoes its share of that one's change. While counting, every number is what's been found so far
 * at one moment, so the result is how many of the folders found so far are ticked.
 */
export function countChosen(choice: Choice, root: Pick<PathCount, "inside" | "done">, counts: readonly PathCount[]): ChoiceCount {
  const paths = ruledPaths(choice);
  let count = choice.all ? root.inside : 0;
  let done = root.done;
  paths.forEach((path, i) => {
    const at = counts[i] ?? { found: false, inside: 0, done: false };
    done &&= at.done;
    const above = takes({ ...choice, rules: withoutRule(choice, path) }, path);
    const on = choice.rules[path];
    if (on !== above) count += (on ? 1 : -1) * ((at.found ? 1 : 0) + at.inside);
  });
  return { count: Math.max(0, Math.min(count, root.inside)), total: root.inside, done };
}

/** The rules without the one on `path`. */
function withoutRule(choice: Choice, path: string): Record<string, boolean> {
  const rules = { ...choice.rules };
  delete rules[path];
  return rules;
}
