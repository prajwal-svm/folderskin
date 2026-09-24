import { useSyncExternalStore } from "react";
import { api, errorMessage, type Skin } from "../lib/tauri";
import { aiFailure } from "../lib/aiError";
import {
  addTurn,
  applyEvent,
  chatId,
  newChat,
  patchTurn,
  persistable,
  readChat,
  renameChat,
  settle,
  upsertSummary,
  type AiEvent,
  type Chat,
  type ChatFolder,
  type ChatRef,
  type ChatSummary,
  type Shape,
  type Turn,
} from "./chats";

/**
 * The assistant's chats for the whole app: the history, the chat that's open, and every request
 * still running, whichever chat it belongs to. It lives outside React so a picture being made
 * carries on while the user looks at the library or another chat, and lands in its own chat when
 * it's done. Each chat is saved (chats.rs) a moment after its requests change; what a request
 * reports while it runs is shown but not saved.
 */

type State = {
  /** The history has been read. */
  ready: boolean;
  list: ChatSummary[];
  /** The chat on screen: saved once it has a request in it. */
  active: Chat | null;
  /** A chat that couldn't be opened or saved, said once. */
  problem: string | null;
  /** This computer is painting, for whichever chat: it paints one picture at a time. */
  localRunning: boolean;
};

let state: State = { ready: false, list: [], active: null, problem: null, localRunning: false };
/** Every chat read or made this session, by id, as it is now. */
const chats = new Map<string, Chat>();
/** The job each running request was started as, by turn id, for Stop. */
const jobs = new Map<string, string>();
/** The running requests the user has pressed Stop on, by turn id: only these end as stopped. */
const stopping = new Set<string>();
/** The running requests being painted on this computer, by turn id. */
const painting = new Set<string>();
const listeners = new Set<() => void>();
let started = false;

function set(next: Partial<State>) {
  state = { ...state, ...next };
  for (const l of listeners) l();
}

// Several reports can arrive in one frame (a step and its log line); the screen follows once.
let queued = false;
function notifySoon() {
  if (queued) return;
  queued = true;
  requestAnimationFrame(() => {
    queued = false;
    const active = state.active ? (chats.get(state.active.id) ?? state.active) : null;
    set({ active });
  });
}

/** Puts `chat` in place, on screen at once when it's the open one. */
function put(chat: Chat, now = true) {
  chats.set(chat.id, chat);
  if (state.active?.id === chat.id) {
    if (now) set({ active: chat });
    else notifySoon();
  }
}

/** Saves the chat `id` as it is now, once it has a request in it. */
function saveNow(id: string): Promise<void> {
  const chat = chats.get(id);
  if (!chat || chat.turns.length === 0) return Promise.resolve();
  return api
    .chatSave(persistable(chat))
    .then((summary) => set({ list: upsertSummary(state.list, summary), problem: null }))
    .catch((e) => set({ problem: `Chats aren't being saved: ${errorMessage(e)}` }));
}

const saving = new Map<string, number>();
/** Saves `chat` a moment from now, once however many changes come before then. */
function saveSoon(id: string) {
  window.clearTimeout(saving.get(id));
  saving.set(
    id,
    window.setTimeout(() => {
      saving.delete(id);
      void saveNow(id);
    }, 250),
  );
}

/** Saves every chat still waiting to be saved, now: the window is closing. */
export async function flushChats(): Promise<void> {
  const ids = [...saving.keys()];
  for (const id of ids) window.clearTimeout(saving.get(id));
  saving.clear();
  await Promise.all(ids.map(saveNow));
}

async function load(id: string): Promise<Chat> {
  const known = chats.get(id);
  if (known) return known;
  const chat = readChat(await api.chatRead(id));
  if (!chat) throw new Error("that chat's file is damaged and can't be opened");
  const settled = settle(chat, Date.now());
  chats.set(id, settled);
  return settled;
}

/**
 * Reads the history, once per session, and opens a new chat unless one is open already (the AI
 * view starts one each time it's shown): every launch starts fresh, and earlier chats wait in the
 * history. A chat is only saved once something has been asked in it, so this never fills the
 * history with empty ones.
 */
export function startChats() {
  if (started) return;
  started = true;
  const fresh = () => state.active ?? newChat(chatId(Date.now()), Date.now());
  api
    .chatsList()
    .then((list) => set({ list, active: fresh(), ready: true }))
    .catch((e) => set({ ready: true, active: fresh(), problem: `Earlier chats couldn't be read: ${errorMessage(e)}` }));
}

export async function openChat(id: string) {
  if (state.active?.id === id) return;
  try {
    set({ active: await load(id) });
  } catch (e) {
    set({ problem: `Couldn't open that chat: ${errorMessage(e)}` });
  }
}

/** A fresh chat, for `folder` if one is given. The open chat stays as it is if nothing's been asked in it yet. */
export function startNewChat(folder: ChatFolder | null) {
  const open = state.active;
  if (open && open.turns.length === 0) {
    put({ ...open, folder });
    return;
  }
  const chat = newChat(chatId(Date.now()), Date.now(), folder);
  chats.set(chat.id, chat);
  set({ active: chat });
}

/** Renames a chat, the open one or any in the history (read first if it hasn't been opened). */
export async function renameChatTo(id: string, title: string) {
  let chat = chats.get(id);
  if (!chat) {
    try {
      chat = await load(id);
    } catch (e) {
      set({ problem: `Couldn't rename that chat: ${errorMessage(e)}` });
      return;
    }
    // Deleted while it was being read.
    if (!state.list.some((s) => s.id === id)) return;
  }
  const next = renameChat(chat, title, Date.now());
  put(next);
  set({ list: state.list.map((s) => (s.id === id ? { ...s, title: next.title } : s)) });
  if (next.turns.length > 0) saveSoon(id);
}

export async function deleteChat(id: string) {
  window.clearTimeout(saving.get(id));
  saving.delete(id);
  try {
    await api.chatDelete(id);
  } catch (e) {
    set({ problem: `Couldn't delete that chat: ${errorMessage(e)}` });
    return;
  }
  chats.delete(id);
  const list = state.list.filter((s) => s.id !== id);
  set({ list });
  if (state.active?.id !== id) return;
  if (list[0]) await openChat(list[0].id);
  else set({ active: newChat(chatId(Date.now()), Date.now()) });
}

/** The folder the open chat's pictures are for. */
export function setChatFolder(folder: ChatFolder | null) {
  const chat = state.active;
  if (!chat || (chat.folder?.path ?? null) === (folder?.path ?? null)) return;
  put({ ...chat, folder, updated: chat.turns.length ? Date.now() : chat.updated });
  if (chat.turns.length) saveSoon(chat.id);
}

/** Copies a picture into the open chat, for its next request. */
export async function keepReference(path: string): Promise<ChatRef> {
  const chat = state.active;
  if (!chat) throw new Error("no chat is open");
  return api.chatKeepReference(chat.id, path);
}

export type Ask = {
  idea: string;
  shape: Shape;
  provider: string;
  model: string;
  /** "OpenAI · GPT Image 2.5", as the request is made. */
  where: string;
  /** Painted on this computer, which paints one picture at a time. */
  local: boolean;
  refs: ChatRef[];
  tags: string[];
  size: string | null;
};

/**
 * Sends a request from the open chat. It runs to the end whatever happens on screen: `onSkin`
 * gets the picture when it's made (App puts it in the library), and the chat it came from shows
 * how it went. False when it can't be sent now: this computer is still painting another.
 */
export function ask(req: Ask, onSkin: (skin: Skin) => void): boolean {
  const chat = state.active;
  if (!chat || (req.local && painting.size > 0)) return false;
  const now = Date.now();
  const turn: Turn = {
    id: `t${now.toString(36)}${Math.floor(Math.random() * 1296).toString(36)}`,
    idea: req.idea,
    shape: req.shape,
    provider: req.provider,
    model: req.model,
    where: req.where,
    refs: req.refs,
    status: "working",
    started: now,
  };
  const chatIdNow = chat.id;
  put(addTurn(chat, turn, now));
  saveSoon(chatIdNow);
  const job = `${chatIdNow}-${turn.id}`;
  jobs.set(turn.id, job);
  if (req.local) {
    painting.add(turn.id);
    set({ localRunning: true });
  }

  const update = (patch: (t: Turn) => Partial<Turn>, immediate: boolean) => {
    const c = chats.get(chatIdNow);
    const t = c?.turns.find((x) => x.id === turn.id);
    if (!c || !t) return;
    put(patchTurn(c, turn.id, patch(t), Date.now()), immediate);
  };
  const onEvent = (event: AiEvent) => update((t) => applyEvent(t, event), false);
  /** A finished turn keeps nothing of what it reported while it ran. */
  const ended = () => ({ finished: Date.now(), stage: undefined, step: undefined, download: undefined });

  api
    .aiGenerate(
      {
        provider: req.provider,
        model: req.model,
        idea: req.idea,
        shape: req.shape,
        size: req.size,
        reference_path: req.refs[0]?.path ?? null,
        reference_paths: req.refs.map((r) => r.path),
        tags: req.tags,
        job,
      },
      onEvent,
    )
    .then((skin) => {
      update(() => ({ ...ended(), status: "done", skinId: skin.id }), true);
      onSkin(skin);
    })
    .catch((e) => {
      // Only the user's Stop ends a request as stopped. Anything else that ends it early is a
      // failure to show, whatever its words say.
      if (stopping.has(turn.id)) return update(() => ({ ...ended(), status: "stopped", error: undefined }), true);
      const error = aiFailure(e);
      update(() => ({ ...ended(), status: "error", error: error.code === "stopped" ? { ...error, code: "failed" } : error }), true);
    })
    .finally(() => {
      jobs.delete(turn.id);
      stopping.delete(turn.id);
      if (painting.delete(turn.id)) set({ localRunning: painting.size > 0 });
      saveSoon(chatIdNow);
    });
  return true;
}

/** Stops a running request. It says "Stopping" until the run has actually let go. */
export function stop(turnId: string) {
  const job = jobs.get(turnId);
  if (!job) return;
  stopping.add(turnId);
  const chatIdNow = job.split("-")[0];
  const c = chats.get(chatIdNow);
  if (c) put(patchTurn(c, turnId, { stage: "Stopping", step: undefined, download: undefined }, Date.now()));
  api.aiCancel(job).catch(() => {
    // Older builds can't stop a run; it finishes, and its picture is kept (or its error shown).
    stopping.delete(turnId);
  });
}

export function dismissProblem() {
  set({ problem: null });
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export function useChats(): State {
  return useSyncExternalStore(subscribe, () => state);
}
