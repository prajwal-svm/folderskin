import { beforeEach, describe, expect, it, vi } from "vitest";

/** What the app has waiting, which a take hands over once. */
let waiting: { pack: string | null; skin: string | null } | null = null;
const pack = (id: string) => ({ pack: id, skin: null });
let inTauri = true;
/** What the page listens to, and what finishing `listen` waits for. */
const listeners = new Map<string, () => void>();
let listening: Promise<void> = Promise.resolve();
const unlistened: string[] = [];

vi.mock("./devMock", () => ({ isTauri: () => inTauri }));
vi.mock("./tauri", () => ({
  api: {
    takeInstallLink: async () => {
      const link = waiting;
      waiting = null;
      return link;
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
    waiting = pack("classic-art");
    const opened: unknown[] = [];
    const stop = watchInstallLinks((link) => opened.push(link));
    await settle();
    expect(opened).toEqual([pack("classic-art")]);

    const skin = { pack: "colours", skin: "a".repeat(64) };
    waiting = skin;
    listeners.get(INSTALL_LINK_EVENT)?.();
    await settle();
    // Told twice about one link: it's taken once.
    listeners.get(INSTALL_LINK_EVENT)?.();
    await settle();
    expect(opened).toEqual([pack("classic-art"), skin]);

    stop();
    expect(listeners.size).toBe(0);
  });

  it("leaves a link waiting when it stops before it listens, for the watcher that comes next", async () => {
    let listen: () => void = () => {};
    listening = new Promise((go) => (listen = go));
    waiting = pack("colours");
    const first: unknown[] = [];
    // React in development: started, stopped and started again straight away.
    const stop = watchInstallLinks((link) => first.push(link));
    stop();
    const second: unknown[] = [];
    watchInstallLinks((link) => second.push(link));
    listen();
    await settle();
    expect(first).toEqual([]);
    expect(second).toEqual([pack("colours")]);
    expect(unlistened).toEqual([INSTALL_LINK_EVENT]);
  });

  it("takes the preview's stand-in link once, with nothing to listen to", async () => {
    inTauri = false;
    waiting = pack("colours");
    const opened: unknown[] = [];
    watchInstallLinks((link) => opened.push(link));
    watchInstallLinks((link) => opened.push(link));
    await settle();
    expect(opened).toEqual([pack("colours")]);
    expect(listeners.size).toBe(0);
  });
});
