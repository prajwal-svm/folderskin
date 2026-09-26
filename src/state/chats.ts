/**
 * Conversations with the AI assistant: what was asked, with which model and pictures, how each
 * request went and what it made. Pure, so every rule is unit-tested; chatStore.ts keeps them and
 * saves them through the app (chats.rs), so they're there the next time the app opens.
 */
import { t } from "../i18n";

/** What's made on a shape that has a base: the whole of it ("folder"), or just the art ("skin"). */
export type Shape = "folder" | "skin";

/** What a reference picture is for: who or what to paint, a look to match, or colours to use. */
export type RefRole = "subject" | "style" | "palette";

/**
 * A reference picture given to a chat, copied into the chat's own folder so it outlives the
 * original, with what it's for (a subject when it doesn't say).
 */
export type ChatRef = { id: string; name: string; path: string; thumb: string; role?: RefRole };

/** A picture's role, a subject when it names none this build knows. */
export const refRole = (r: Pick<ChatRef, "role">): RefRole => (r.role === "style" || r.role === "palette" ? r.role : "subject");

/** What went wrong, in words, with a code the chat can offer the right next step for. */
export type TurnError = { code: string; message: string; fix?: string[]; ask?: string };

/** One request and how it went. `stage`, `step`, `download` and `log` are only while it runs. */
export type Turn = {
  id: string;
  idea: string;
  shape: Shape;
  /** The shape it was made for, by id (src/lib/shapes.ts); none in a chat from before shapes, which was a folder's. */
  base?: string;
  /** The built-in style it was made in, by id (src/lib/styles.ts), when one was picked. */
  style?: string;
  /** The saved prompt whose look it was made in, when one was picked, named as it was then. */
  skill?: { id: string; name: string };
  provider: string;
  model: string;
  /** "OpenAI · GPT Image 2.5", as it was when asked. */
  where: string;
  refs: ChatRef[];
  status: "working" | "done" | "error" | "stopped";
  started: number;
  finished?: number;
  /** The skin it made, in the library. */
  skinId?: string;
  error?: TurnError;
  stage?: string;
  step?: { done: number; total: number };
  download?: { file: string; done: number; total: number };
  log?: string[];
};

export type ChatFolder = { path: string; name: string };

export type Chat = {
  version: 1;
  id: string;
  title: string;
  /** Named by the user, so the first idea doesn't rename it. */
  named: boolean;
  created: number;
  updated: number;
  /** The folder its pictures are meant for. */
  folder: ChatFolder | null;
  /** The shape its pictures are made for, by id, once one is picked; until then the folder the app puts skins on. */
  base?: string;
  turns: Turn[];
};

/** `turns` counts every request, `pictures` only those that made one. */
export type ChatSummary = { id: string; title: string; created: number; updated: number; turns: number; pictures: number; cover: string | null };

/** How many pictures a chat made, as the history list says it. */
export function picturesMade(n: number): string {
  if (n === 0) return t("ai.chats.noPictures");
  return t("ai.chats.pictures", { count: n });
}

/** What an AI run reports while it works (the Tauri channel's messages). */
export type AiEvent =
  | { type: "stage"; stage: string; message: string }
  | { type: "progress"; step: number; steps: number }
  | { type: "download"; file: string; done: number; total: number }
  | { type: "log"; level: "info" | "warn" | "error"; message: string };

/** The title a chat has until its first idea names it. Kept in the chat as it is, and shown as
 *  `ai.chats.newChat` in the language on show (see `chatTitle`). */
export const NEW_TITLE = "New chat";

/** A chat's title as it's shown: an untitled chat's in the language on show. */
export const chatTitle = (title: string) => (title === NEW_TITLE ? t("ai.chats.newChat") : title);
/** How many log lines a running turn keeps, and how many are saved with it. */
export const LOG_LINES = 400;
export const SAVED_LOG_LINES = 40;
const TITLE_CHARS = 48;

/** A chat id: `c` and a time and random part, safe as a file name (chats.rs checks the same). */
export function chatId(now: number, random: () => number = Math.random): string {
  return `c${now.toString(36)}${Math.floor(random() * 36 ** 6)
    .toString(36)
    .padStart(6, "0")}`;
}

export function newChat(id: string, now: number, folder: ChatFolder | null = null): Chat {
  return { version: 1, id, title: NEW_TITLE, named: false, created: now, updated: now, folder, turns: [] };
}

/** A chat's title from its first idea: its first words, cut at a word, capitalised. */
export function titleFrom(idea: string): string {
  const words = idea.replace(/\s+/g, " ").trim();
  if (!words) return NEW_TITLE;
  let title = words;
  if (title.length > TITLE_CHARS) {
    const cut = title.slice(0, TITLE_CHARS + 1);
    const at = cut.lastIndexOf(" ");
    title = (at > 16 ? cut.slice(0, at) : cut.slice(0, TITLE_CHARS)).replace(/[\s,.;:!?-]+$/, "");
  }
  return title.charAt(0).toUpperCase() + title.slice(1);
}

export function addTurn(chat: Chat, turn: Turn, now: number): Chat {
  const first = chat.turns.length === 0 && !chat.named;
  return { ...chat, title: first ? titleFrom(turn.idea) : chat.title, updated: now, turns: [...chat.turns, turn] };
}

export function patchTurn(chat: Chat, id: string, patch: Partial<Turn>, now: number): Chat {
  if (!chat.turns.some((t) => t.id === id)) return chat;
  return { ...chat, updated: now, turns: chat.turns.map((t) => (t.id === id ? { ...t, ...patch } : t)) };
}

export function renameChat(chat: Chat, title: string, now: number): Chat {
  const clean = title.replace(/\s+/g, " ").trim().slice(0, 80);
  return clean ? { ...chat, title: clean, named: true, updated: now } : { ...chat, title: chat.turns[0] ? titleFrom(chat.turns[0].idea) : NEW_TITLE, named: false, updated: now };
}

/** A running turn with what `event` says about it. */
export function applyEvent(turn: Turn, event: AiEvent): Turn {
  switch (event.type) {
    case "stage":
      // A new stage: whatever the last one was counting is done with.
      return { ...turn, stage: event.message, step: undefined, download: undefined };
    case "progress":
      return { ...turn, step: { done: Math.max(0, Math.min(event.step, event.steps)), total: event.steps } };
    case "download":
      return { ...turn, download: { file: event.file, done: event.done, total: event.total } };
    case "log": {
      const line = event.level === "info" ? event.message : `${event.level}: ${event.message}`;
      const log = [...(turn.log ?? []), line];
      return { ...turn, log: log.length > LOG_LINES ? log.slice(-LOG_LINES) : log };
    }
  }
}

/** How far a running turn is, 0 to 1, when it can say; null while it can't. */
export function progressOf(turn: Turn): number | null {
  if (turn.download && turn.download.total > 0) return turn.download.done / turn.download.total;
  if (turn.step && turn.step.total > 0) return turn.step.done / turn.step.total;
  return null;
}

/**
 * A chat as it was saved, made safe to show: a request that was still running when the app closed
 * didn't finish, and says so.
 */
export function settle(chat: Chat, now: number): Chat {
  if (!chat.turns.some((t) => t.status === "working")) return chat;
  return {
    ...chat,
    turns: chat.turns.map((t) =>
      t.status === "working" ? { ...t, status: "stopped", finished: t.finished ?? now, stage: undefined, step: undefined, download: undefined } : t,
    ),
  };
}

/** What's saved of a chat: everything but what only matters while a request runs, and the log's tail. */
export function persistable(chat: Chat): Chat {
  return {
    ...chat,
    turns: chat.turns.map(({ stage: _s, step: _p, download: _d, log, ...t }) => (log && log.length ? { ...t, log: log.slice(-SAVED_LOG_LINES) } : t)),
  };
}

export function summaryOf(chat: Chat): ChatSummary {
  const cover = [...chat.turns].reverse().find((t) => t.skinId)?.skinId ?? null;
  const pictures = chat.turns.filter((t) => t.skinId).length;
  return { id: chat.id, title: chat.title, created: chat.created, updated: chat.updated, turns: chat.turns.length, pictures, cover };
}

/** Newest first, the one just changed replacing its old self. */
export function upsertSummary(list: ChatSummary[], summary: ChatSummary): ChatSummary[] {
  return [summary, ...list.filter((s) => s.id !== summary.id)].sort((a, b) => b.updated - a.updated);
}

const DAY = 24 * 60 * 60 * 1000;

/** The chats grouped the way people look for them: today, yesterday, this week, before that. */
export function groupChats(list: ChatSummary[], now: number): { id: string; label: string; chats: ChatSummary[] }[] {
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);
  const today = start.getTime();
  const groups = [
    { id: "today", from: today },
    { id: "yesterday", from: today - DAY },
    { id: "week", from: today - 7 * DAY },
    { id: "earlier", from: -Infinity },
  ] as const;
  const out = groups.map((g) => ({ id: g.id, label: t(`ai.chats.groups.${g.id}`), chats: [] as ChatSummary[] }));
  for (const chat of [...list].sort((a, b) => b.updated - a.updated)) {
    const i = groups.findIndex((g) => chat.updated >= g.from);
    out[i].chats.push(chat);
  }
  return out.filter((g) => g.chats.length > 0);
}

/** The chats whose title has every word of `query` in it. */
export function searchChats(list: ChatSummary[], query: string): ChatSummary[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return list;
  return list.filter((c) => {
    const title = c.title.toLowerCase();
    return words.every((w) => title.includes(w));
  });
}

/** Reads a chat the app saved, or null for anything that isn't one. */
export function readChat(raw: unknown): Chat | null {
  if (!raw || typeof raw !== "object") return null;
  const c = raw as Partial<Chat>;
  if (typeof c.id !== "string" || !Array.isArray(c.turns)) return null;
  const num = (v: unknown, fallback: number) => (typeof v === "number" && Number.isFinite(v) ? v : fallback);
  const id = (v: unknown) => (typeof v === "string" && v ? v : undefined);
  const turns = c.turns.filter((t): t is Turn => !!t && typeof t === "object" && typeof (t as Turn).id === "string" && typeof (t as Turn).idea === "string");
  return {
    version: 1,
    id: c.id,
    title: typeof c.title === "string" && c.title ? c.title : NEW_TITLE,
    named: c.named === true,
    created: num(c.created, 0),
    updated: num(c.updated, 0),
    folder: c.folder && typeof c.folder.path === "string" && typeof c.folder.name === "string" ? { path: c.folder.path, name: c.folder.name } : null,
    base: id(c.base),
    turns: turns.map((t) => ({
      ...t,
      refs: Array.isArray(t.refs) ? t.refs : [],
      shape: t.shape === "skin" ? "skin" : "folder",
      base: id(t.base),
      style: id(t.style),
      skill: t.skill && typeof t.skill === "object" && id(t.skill.id) && typeof t.skill.name === "string" ? { id: t.skill.id, name: t.skill.name } : undefined,
    })),
  };
}
