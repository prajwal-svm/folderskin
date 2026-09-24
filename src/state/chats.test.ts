import { describe, expect, it } from "vitest";
import {
  addTurn,
  applyEvent,
  chatId,
  groupChats,
  LOG_LINES,
  newChat,
  NEW_TITLE,
  patchTurn,
  persistable,
  picturesMade,
  progressOf,
  readChat,
  renameChat,
  SAVED_LOG_LINES,
  searchChats,
  settle,
  summaryOf,
  titleFrom,
  upsertSummary,
  type Turn,
} from "./chats";

const turn = (id: string, idea = "a lighthouse at dusk", extra: Partial<Turn> = {}): Turn => ({
  id,
  idea,
  shape: "folder",
  provider: "openai",
  model: "gpt-image",
  where: "OpenAI · GPT Image",
  refs: [],
  status: "working",
  started: 1,
  ...extra,
});

describe("chatId", () => {
  it("is a safe file name, different each time", () => {
    const a = chatId(1790000000000, () => 0.1);
    const b = chatId(1790000000000, () => 0.2);
    expect(a).toMatch(/^c[a-z0-9]{10,}$/);
    expect(a).not.toBe(b);
  });
});

describe("titleFrom", () => {
  it("takes the first idea, capitalised", () => {
    expect(titleFrom("  a  lighthouse at dusk ")).toBe("A lighthouse at dusk");
    expect(titleFrom("")).toBe(NEW_TITLE);
  });

  it("cuts a long idea at a word", () => {
    const t = titleFrom("an enormous victorian greenhouse full of ferns and orchids, lit by lanterns at night");
    expect(t.length).toBeLessThanOrEqual(48);
    expect(t).toBe("An enormous victorian greenhouse full of ferns");
    expect(t).not.toMatch(/…|[\s,.;:!?-]$/);
  });
});

describe("a chat's turns", () => {
  it("names the chat after its first idea, unless the user named it", () => {
    const c = addTurn(newChat("c1", 0), turn("t1"), 5);
    expect(c.title).toBe("A lighthouse at dusk");
    expect(c.updated).toBe(5);
    const named = renameChat(newChat("c2", 0), "Beach set", 1);
    expect(addTurn(named, turn("t1"), 2).title).toBe("Beach set");
    expect(addTurn(c, turn("t2", "another idea"), 6).title).toBe("A lighthouse at dusk");
  });

  it("renaming to nothing goes back to the automatic title", () => {
    const c = addTurn(renameChat(newChat("c1", 0), "Mine", 1), turn("t1"), 2);
    expect(renameChat(c, "   ", 3)).toMatchObject({ title: "A lighthouse at dusk", named: false });
  });

  it("patches only the turn asked for", () => {
    const c = addTurn(addTurn(newChat("c1", 0), turn("t1"), 1), turn("t2"), 2);
    const done = patchTurn(c, "t2", { status: "done", skinId: "s1" }, 3);
    expect(done.turns.map((t) => t.status)).toEqual(["working", "done"]);
    expect(patchTurn(c, "nope", { status: "done" }, 3)).toBe(c);
  });
});

describe("applyEvent", () => {
  it("follows stages, steps and downloads, a new stage clearing the last one's count", () => {
    let t = applyEvent(turn("t"), { type: "download", file: "model.gguf", done: 5, total: 10 });
    expect(progressOf(t)).toBe(0.5);
    t = applyEvent(t, { type: "stage", stage: "paint", message: "Painting" });
    expect(t.stage).toBe("Painting");
    expect(progressOf(t)).toBeNull();
    t = applyEvent(t, { type: "progress", step: 3, steps: 4 });
    expect(progressOf(t)).toBe(0.75);
    expect(applyEvent(t, { type: "progress", step: 9, steps: 4 }).step).toEqual({ done: 4, total: 4 });
  });

  it("keeps the last lines of the log, marking warnings", () => {
    let t = turn("t");
    for (let i = 0; i < LOG_LINES + 5; i++) t = applyEvent(t, { type: "log", level: "info", message: `line ${i}` });
    t = applyEvent(t, { type: "log", level: "warn", message: "low memory" });
    expect(t.log).toHaveLength(LOG_LINES);
    expect(t.log?.at(-1)).toBe("warn: low memory");
  });
});

describe("saving and opening", () => {
  it("saves no running state and only the log's tail", () => {
    const log = Array.from({ length: 100 }, (_, i) => `l${i}`);
    const c = addTurn(newChat("c1", 0), turn("t1", "x", { stage: "Painting", step: { done: 1, total: 2 }, log }), 1);
    const saved = persistable(c).turns[0];
    expect(saved.stage).toBeUndefined();
    expect(saved.step).toBeUndefined();
    expect(saved.log).toHaveLength(SAVED_LOG_LINES);
  });

  it("a request still running when the app closed opens as stopped", () => {
    const c = addTurn(newChat("c1", 0), turn("t1"), 1);
    const s = settle(c, 9);
    expect(s.turns[0]).toMatchObject({ status: "stopped", finished: 9 });
    const quiet = patchTurn(c, "t1", { status: "done" }, 2);
    expect(settle(quiet, 9)).toBe(quiet);
  });

  it("reads back what it saved and refuses what isn't a chat", () => {
    const c = addTurn(newChat("c1", 0, { path: "/a/b", name: "b" }), turn("t1"), 1);
    expect(readChat(JSON.parse(JSON.stringify(persistable(c))))).toEqual(persistable(c));
    expect(readChat(null)).toBeNull();
    expect(readChat({ id: 3, turns: [] })).toBeNull();
    expect(readChat({ id: "c", turns: [{ nope: true }] })?.turns).toEqual([]);
  });
});

describe("the list of chats", () => {
  it("summarises a chat with its latest picture as the cover", () => {
    let c = addTurn(newChat("c1", 0), turn("t1", "a", { status: "done", skinId: "s1" }), 1);
    c = addTurn(c, turn("t2", "b", { status: "error" }), 2);
    expect(summaryOf(c)).toMatchObject({ id: "c1", turns: 2, pictures: 1, cover: "s1" });
  });

  it("counts the pictures a chat made, not the requests that were stopped or failed", () => {
    let c = addTurn(newChat("c1", 0), turn("t1", "a red kite", { status: "stopped" }), 1);
    c = addTurn(c, turn("t2", "a red kite", { status: "stopped" }), 2);
    expect(summaryOf(c)).toMatchObject({ turns: 2, pictures: 0 });
    expect(picturesMade(0)).toBe("no pictures");
    expect(picturesMade(1)).toBe("1 picture");
    expect(picturesMade(2)).toBe("2 pictures");
  });

  it("keeps newest first and one entry per chat", () => {
    const a = { id: "a", title: "A", created: 0, updated: 1, turns: 1, pictures: 0, cover: null };
    const b = { id: "b", title: "B", created: 0, updated: 2, turns: 1, pictures: 0, cover: null };
    expect(upsertSummary([b, a], { ...a, updated: 3 }).map((s) => s.id)).toEqual(["a", "b"]);
  });

  it("groups by day", () => {
    const now = new Date(2026, 8, 23, 15, 0).getTime();
    const at = (d: number, h = 12) => new Date(2026, 8, 23 - d, h).getTime();
    const list = [
      { id: "t", title: "t", created: 0, updated: at(0, 9), turns: 1, pictures: 0, cover: null },
      { id: "y", title: "y", created: 0, updated: at(1), turns: 1, pictures: 0, cover: null },
      { id: "w", title: "w", created: 0, updated: at(4), turns: 1, pictures: 0, cover: null },
      { id: "o", title: "o", created: 0, updated: at(40), turns: 1, pictures: 0, cover: null },
    ];
    expect(groupChats(list, now).map((g) => [g.label, g.chats.map((c) => c.id)])).toEqual([
      ["Today", ["t"]],
      ["Yesterday", ["y"]],
      ["Previous 7 days", ["w"]],
      ["Earlier", ["o"]],
    ]);
  });

  it("finds chats by every word of their title", () => {
    const list = [
      { id: "1", title: "Lighthouse at dusk", created: 0, updated: 0, turns: 1, pictures: 0, cover: null },
      { id: "2", title: "Pop art cats", created: 0, updated: 0, turns: 1, pictures: 0, cover: null },
    ];
    expect(searchChats(list, "dusk light").map((c) => c.id)).toEqual(["1"]);
    expect(searchChats(list, " ")).toBe(list);
  });
});
