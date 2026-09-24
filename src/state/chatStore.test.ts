import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { summaryOf, type Chat, type Turn } from "./chats";
import type { Ask } from "./chatStore";

// The app's commands, stood in for: each test says what they give back.
const api = vi.hoisted(() => ({
  chatsList: vi.fn(),
  chatRead: vi.fn(),
  chatSave: vi.fn(),
  chatDelete: vi.fn(),
  chatKeepReference: vi.fn(),
  aiGenerate: vi.fn(),
  aiCancel: vi.fn(),
}));

vi.mock("../lib/tauri", () => ({
  api,
  errorMessage: (e: unknown) => (e instanceof Error ? e.message : String(e)),
}));

let store: typeof import("./chatStore");

/** Lets every promise that can settle now settle (the save timer is faked, so it waits). */
const settled = () => new Promise((r) => setImmediate(r));

const saved = (id: string, title: string, updated: number): Chat => {
  const turn: Turn = { id: "t1", idea: title, shape: "folder", provider: "openai", model: "gpt-image", where: "OpenAI · GPT Image", refs: [], status: "done", started: updated, finished: updated };
  return { version: 1, id, title, named: false, created: updated, updated, folder: null, turns: [turn] };
};

const request = (extra: Partial<Ask> = {}): Ask => ({
  idea: "a paper boat",
  shape: "folder",
  provider: "openai",
  model: "gpt-image",
  where: "OpenAI · GPT Image",
  local: false,
  refs: [],
  tags: [],
  size: null,
  ...extra,
});
const onDevice = (extra: Partial<Ask> = {}) => request({ provider: "local", model: "flux", where: "This computer · FLUX.2 klein", local: true, ...extra });

/** A request that runs until the test ends it. */
function pending() {
  let finish!: (skin: unknown) => void;
  let fail!: (e: unknown) => void;
  const done = new Promise((resolve, reject) => {
    finish = resolve;
    fail = reject;
  });
  return { done, finish, fail };
}

/** Starts the store on `chats` (newest first), the first of them open. */
async function start(chats: Chat[] = []) {
  api.chatsList.mockResolvedValue(chats.map(summaryOf));
  api.chatRead.mockImplementation(async (id: string) => {
    const chat = chats.find((c) => c.id === id);
    if (!chat) throw "that chat isn't on this computer any more";
    return chat;
  });
  store.startChats();
  await settled();
}

/** The last time `id` was saved, as it was saved. */
const lastSaved = (id: string): Chat | undefined =>
  api.chatSave.mock.calls
    .map(([c]) => c as Chat)
    .filter((c) => c.id === id)
    .at(-1);

/** The chat and turn a request was started for, from the job it was named. */
const jobOf = (call = 0): [string, string] => api.aiGenerate.mock.calls[call][0].job.split("-");

beforeEach(async () => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  vi.stubGlobal("window", globalThis);
  vi.stubGlobal("requestAnimationFrame", (cb: (t: number) => void) => setTimeout(() => cb(0), 0));
  for (const f of Object.values(api)) f.mockReset();
  api.chatSave.mockImplementation(async (chat: Chat) => summaryOf(chat));
  api.aiCancel.mockResolvedValue(undefined);
  vi.resetModules();
  store = await import("./chatStore");
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("renaming a chat", () => {
  it("renames one from the history that hasn't been opened", async () => {
    await start([saved("cnewer", "A lighthouse", 2), saved("colder", "Older chat", 1)]);
    await store.renameChatTo("colder", "Renamed");
    await vi.advanceTimersByTimeAsync(250);
    expect(lastSaved("colder")).toMatchObject({ title: "Renamed", named: true, turns: [{ id: "t1", status: "done" }] });
  });
});

describe("how a request ends", () => {
  it("is an error when it fails on its own, even if the words say it was cancelled", async () => {
    await start();
    api.aiGenerate.mockRejectedValue("Google returned an error (499): The operation was cancelled.");
    store.ask(request({ provider: "google" }), vi.fn());
    await settled();
    await vi.advanceTimersByTimeAsync(250);
    const turn = lastSaved(jobOf()[0])?.turns[0];
    expect(turn?.status).toBe("error");
    expect(turn?.error).toMatchObject({ code: "failed", message: "Google returned an error (499): The operation was cancelled." });
  });

  it("is stopped when the user pressed Stop", async () => {
    await start();
    const run = pending();
    api.aiGenerate.mockReturnValue(run.done);
    store.ask(request(), vi.fn());
    const [chatId, turnId] = jobOf();
    store.stop(turnId);
    expect(api.aiCancel).toHaveBeenCalledWith(`${chatId}-${turnId}`);
    run.fail({ code: "stopped", message: "Stopped before it finished." });
    await settled();
    await vi.advanceTimersByTimeAsync(250);
    const turn = lastSaved(chatId)?.turns[0];
    expect(turn?.status).toBe("stopped");
    expect(turn?.error).toBeUndefined();
  });
});

describe("this computer", () => {
  it("paints one picture at a time, whichever chat asks", async () => {
    await start();
    const first = pending();
    api.aiGenerate.mockReturnValueOnce(first.done);
    expect(store.ask(onDevice(), vi.fn())).toBe(true);
    store.startNewChat(null);
    expect(store.ask(onDevice({ idea: "a second boat" }), vi.fn())).toBe(false);
    expect(api.aiGenerate).toHaveBeenCalledTimes(1);
    // A provider's servers aren't this computer: they run alongside.
    api.aiGenerate.mockReturnValueOnce(pending().done);
    expect(store.ask(request(), vi.fn())).toBe(true);
    first.finish({ id: "user:ai1" });
    await settled();
    api.aiGenerate.mockReturnValueOnce(pending().done);
    expect(store.ask(onDevice({ idea: "a third boat" }), vi.fn())).toBe(true);
    expect(api.aiGenerate).toHaveBeenCalledTimes(3);
  });
});

describe("closing the window", () => {
  it("saves at once what was waiting to be saved", async () => {
    await start();
    api.aiGenerate.mockResolvedValue({ id: "user:ai1" });
    store.ask(request(), vi.fn());
    await settled();
    await store.flushChats();
    expect(lastSaved(jobOf()[0])?.turns[0]).toMatchObject({ status: "done", skinId: "user:ai1" });
    // Nothing is left to save a second time.
    const saves = api.chatSave.mock.calls.length;
    await vi.advanceTimersByTimeAsync(250);
    expect(api.chatSave).toHaveBeenCalledTimes(saves);
  });
});
