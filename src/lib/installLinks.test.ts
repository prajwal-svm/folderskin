import { beforeEach, describe, expect, it, vi } from "vitest";

/** The pack the app has waiting, which a take hands over once. */
let waiting: string | null = null;
let inTauri = true;
/** What the page listens to, and what finishing `listen` waits for. */
const listeners = new Map<string, () => void>();
let listening: Promise<void> = Promise.resolve();
const unlistened: string[] = [];

vi.mock("./devMock", () => ({ isTauri: () => inTauri }));
vi.mock("./tauri", () => ({
  api: {
    takeInstallLink: async () => {
      const pack = waiting;
      waiting = null;
      return pack;
    },
  },
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: async (event: string, handler: () => void) => {
    await listening;
    listeners.set(event, handler);
    return () => {
      listeners.delete(event);
      unlistened.push(event);
    };
  },
}));

const { INSTALL_LINK_EVENT, watchInstallLinks } = await import("./installLinks");

const settle = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  waiting = null;
  inTauri = true;
  listeners.clear();
  listening = Promise.resolve();
  unlistened.length = 0;
});

describe("install links", () => {
  it("hands over the link the app was opened with, then each one after it, once each", async () => {
    waiting = "classic-art";
    const opened: string[] = [];
    const stop = watchInstallLinks((id) => opened.push(id));
    await settle();
    expect(opened).toEqual(["classic-art"]);

    waiting = "colours";
    listeners.get(INSTALL_LINK_EVENT)?.();
    await settle();
    // Told twice about one link: it's taken once.
    listeners.get(INSTALL_LINK_EVENT)?.();
    await settle();
    expect(opened).toEqual(["classic-art", "colours"]);

    stop();
    expect(listeners.size).toBe(0);
  });

  it("leaves a link waiting when it stops before it listens, for the watcher that comes next", async () => {
    let listen: () => void = () => {};
    listening = new Promise((go) => (listen = go));
    waiting = "colours";
    const first: string[] = [];
    // React in development: started, stopped and started again straight away.
    const stop = watchInstallLinks((id) => first.push(id));
    stop();
    const second: string[] = [];
    watchInstallLinks((id) => second.push(id));
    listen();
    await settle();
    expect(first).toEqual([]);
    expect(second).toEqual(["colours"]);
    expect(unlistened).toEqual([INSTALL_LINK_EVENT]);
  });

  it("takes the preview's stand-in link once, with nothing to listen to", async () => {
    inTauri = false;
    waiting = "colours";
    const opened: string[] = [];
    watchInstallLinks((id) => opened.push(id));
    watchInstallLinks((id) => opened.push(id));
    await settle();
    expect(opened).toEqual(["colours"]);
    expect(listeners.size).toBe(0);
  });
});
