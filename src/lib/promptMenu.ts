/**
 * The menus the AI prompt opens as it's typed in: "@" for the shapes a picture can be for, "/"
 * for the built-in styles, the ideas to start from and the user's own prompts. Pure, so every rule
 * here is unit-tested; PromptMenu.tsx draws them and PromptBox.tsx drives them from the keys.
 */
import { fold, matchScore } from "./shapes";
import type { Look } from "./styles";

/** What opens a menu: "@" or "/", at the start of the box or of a word. */
export type TriggerKind = "@" | "/";

/** A trigger being typed: its kind, what's typed after it, and where it is in the text. */
export type Trigger = { kind: TriggerKind; query: string; start: number; end: number };

/**
 * The trigger the caret is in, when it's in one: "@" or "/" at the start of the text or after a
 * space, and what follows it up to the caret, with no space in it. "a fox /wood|" is a "/" asking
 * for "wood"; "and/or", a link's "https://" and an email's "@" aren't triggers.
 */
export function findTrigger(text: string, caret: number): Trigger | null {
  const before = text.slice(0, caret);
  const m = /(^|\s)([@/])(\S*)$/u.exec(before);
  if (!m) return null;
  const start = before.length - m[2].length - m[3].length;
  return { kind: m[2] as TriggerKind, query: m[3], start, end: caret };
}

/** `text` with the trigger taken out, and the spaces either side of it made one. */
export function removeTrigger(text: string, trigger: Trigger): { text: string; caret: number } {
  return replaceTrigger(text, trigger, "");
}

/**
 * `text` with the trigger replaced by `insert`, spaced from the words either side, and where the
 * caret goes after it: after what went in, ready for the next word. With nothing else in the
 * box, `insert` is the whole of it.
 */
export function replaceTrigger(text: string, trigger: Trigger, insert: string): { text: string; caret: number } {
  const head = text.slice(0, trigger.start).replace(/\s+$/u, "");
  const tail = text.slice(trigger.end).replace(/^\s+/u, "");
  const middle = insert.trim();
  const before = [head, middle].filter(Boolean).join(" ");
  if (tail) return { text: before ? `${before} ${tail}` : tail, caret: before ? before.length + 1 : 0 };
  // Words before a trigger that's gone keep a space after them, for the words that come next.
  const out = before && !middle ? `${before} ` : before;
  return { text: out, caret: out.length };
}

/** One row of a menu. */
export type MenuItem =
  | { kind: "shape"; id: string; name: string; note: string; thumb: string | null; current: boolean }
  | { kind: "prompt"; id: string; name: string; note: string; look: Look | null }
  | { kind: "style"; id: string; name: string; note: string; current: boolean }
  | { kind: "idea"; id: string; name: string; note: string }
  | { kind: "save"; id: "save"; name: string; note: string };

/** A group of rows under a heading; `title` is empty for a group with none. */
export type MenuGroup = { id: string; title: string; items: MenuItem[] };

type Named = { id: string; name: string; note: string };

/** `list` narrowed to what `query` matches, best first, keeping the order among equals. */
export function narrow<T extends Named>(list: T[], query: string, more: (item: T) => string = () => ""): T[] {
  return list
    .map((item, i) => ({ item, i, score: matchScore(query, item.name, `${item.note} ${more(item)}`) }))
    .filter((s) => s.score > 0)
    .sort((a, b) => b.score - a.score || a.i - b.i)
    .map((s) => s.item);
}

/** How many of each a "/" menu shows while nothing is typed: enough to see what's there. */
export const IDLE_LIMIT = { prompt: 6, idea: 8 } as const;

/**
 * The "/" menu for `query`: the user's own prompts first, then the styles under their headings,
 * then the ideas, each narrowed to what's typed, and at the end a row to save what's in the box as
 * a prompt when there's something to save. Empty groups are left out.
 */
export function slashMenu({
  query,
  prompts,
  styles,
  ideas,
  canSave,
  titles,
  save,
}: {
  query: string;
  prompts: (Named & { look: Look | null; text: string })[];
  /** The styles, each group under its heading. */
  styles: { id: string; title: string; items: (Named & { current: boolean })[] }[];
  ideas: Named[];
  canSave: boolean;
  titles: { prompts: string; ideas: string };
  save: { name: string; note: string };
}): MenuGroup[] {
  const typed = query.trim() !== "";
  const cap = <T>(list: T[], n: number) => (typed ? list : list.slice(0, n));
  /** How well a group's best row matches what's typed. */
  const best = <T extends Named>(list: T[], more: (item: T) => string = () => "") => Math.max(0, ...list.map((i) => matchScore(query, i.name, `${i.note} ${more(i)}`)));
  const promptRows = cap(narrow(prompts, query, (p) => p.text), IDLE_LIMIT.prompt);
  const ideaRows = cap(narrow(ideas, query), IDLE_LIMIT.idea);
  let groups: (MenuGroup & { best: number })[] = [
    {
      id: "prompts",
      title: titles.prompts,
      best: best(promptRows, (p) => p.text),
      items: promptRows.map((p) => ({ kind: "prompt" as const, id: p.id, name: p.name, note: p.note, look: p.look })),
    },
    ...styles.map((g) => {
      const rows = narrow(g.items, query);
      return {
        id: `styles-${g.id}`,
        title: g.title,
        best: best(rows),
        items: rows.map((s) => ({ kind: "style" as const, id: s.id, name: s.name, note: s.note, current: s.current })),
      };
    }),
    { id: "ideas", title: titles.ideas, best: best(ideaRows), items: ideaRows.map((i) => ({ kind: "idea" as const, id: i.id, name: i.name, note: i.note })) },
  ];
  // Once something is typed, the groups whose names match it come before those whose words only
  // mention it: "/light" finds Lighthouse before every style lit by studio light.
  if (typed) groups = groups.map((g, i) => ({ g, i })).sort((a, b) => b.g.best - a.g.best || a.i - b.i).map(({ g }) => g);
  const out: MenuGroup[] = groups.map(({ id, title, items }) => ({ id, title, items }));
  // Saving is offered whatever's typed after the "/", as what's typed is often its name: first when
  // nothing is typed yet, where it's seen without scrolling, and last once something is, so Enter
  // takes the best match rather than starting to save.
  if (canSave) {
    const row: MenuGroup = { id: "save", title: "", items: [{ kind: "save", id: "save", name: save.name, note: save.note }] };
    if (typed) out.push(row);
    else out.unshift(row);
  }
  return out.filter((g) => g.items.length > 0);
}

/** Every row of `groups`, in order: what the arrow keys move through. */
export const rowsOf = (groups: MenuGroup[]): MenuItem[] => groups.flatMap((g) => g.items);

/** The row `step` rows from `at` among `count`, going round at either end; 0 when there are none. */
export function moveActive(at: number, step: number, count: number): number {
  if (count <= 0) return 0;
  return (((at + step) % count) + count) % count;
}

/** A prompt's first words, for its name when it's saved: at most `max` characters, cut at a word. */
export function suggestName(text: string, max = 32): string {
  const words = text.replace(/\s+/g, " ").trim();
  if (words.length <= max) return words.replace(/[\s,.;:!?-]+$/u, "");
  const cut = words.slice(0, max + 1);
  const at = cut.lastIndexOf(" ");
  return (at > 8 ? cut.slice(0, at) : cut.slice(0, max)).replace(/[\s,.;:!?-]+$/u, "");
}

/**
 * Who `text` names as its style: a capitalised name after "by" or "in the style of", as
 * folderskin_ai::skill::names_someone finds it. A prompt that does can be saved, with a word first:
 * a style made from a living artist's name can't go in a pack, and describing the technique works
 * better anyway.
 */
export function namesSomeone(text: string): string | null {
  const words = text.split(/\s+/).filter(Boolean);
  const capital = (w: string) => /^\p{Lu}/u.test(w) && (w.match(/\p{L}/gu)?.length ?? 0) > 1;
  for (let i = 0; i < words.length; i++) {
    const w = words[i].toLowerCase();
    let at = -1;
    if (w === "by") at = i + 1;
    else if (w === "style" && i >= 2 && words[i - 2].toLowerCase() === "in" && words[i - 1].toLowerCase() === "the" && words[i + 1]?.toLowerCase() === "of") at = i + 2;
    if (at < 0) continue;
    const name: string[] = [];
    for (let j = at; j < words.length && capital(words[j]); j++) name.push(words[j].replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}]+$/gu, ""));
    if (name.length) return name.join(" ");
  }
  return null;
}

/** Whether `name` is taken by one of `prompts`, whatever its capitals: saving under it replaces that one. */
export function takenBy<T extends { name: string }>(prompts: T[], name: string): T | undefined {
  const n = fold(name.replace(/\s+/g, " ").trim());
  return n ? prompts.find((p) => fold(p.name) === n) : undefined;
}
