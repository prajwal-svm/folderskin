import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { summaryOf, type Chat } from "./chats";
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

const request = (extra: Partial<Ask> = {}): Ask => ({
  idea: "a paper boat",
  shape: "folder",
  provider: "openai",
  model: "gpt-image",
  where: "OpenAI · GPT Image",
  refs: [],
  tags: [],
  size: null,
  ...extra,
});

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
