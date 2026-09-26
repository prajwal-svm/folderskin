import { describe, expect, it } from "vitest";
import { check, countChosen, everything, forRun, isEverything, isNothing, pathIn, ruledPaths, set, setAll, takes, toggle, type Choice } from "./folderChoice";
import type { PathCount } from "./tree";

const ROOT = "/work/Projects";
const at = (path: string) => `${ROOT}/${path}`;
const start = () => everything(ROOT, "/");

/** Each named folder's box. */
const checks = (choice: Choice, paths: string[]) => Object.fromEntries(paths.map((p) => [p, check(choice, at(p))]));
/** Whether each named folder is ticked itself. */
const taken = (choice: Choice, paths: string[]) => Object.fromEntries(paths.map((p) => [p, takes(choice, at(p))]));

describe("choosing folders inside a folder, as rules", () => {
  it("starts with every folder ticked, as including subfolders takes them all", () => {
    const choice = start();
    expect(isEverything(choice)).toBe(true);
    expect(isNothing(choice)).toBe(false);
    expect(checks(choice, ["Clients", "Photos/2021/Holiday"])).toEqual({ Clients: "on", "Photos/2021/Holiday": "on" });
    expect(forRun(choice)).toEqual({ all: true, rules: [] });
  });

  it("ticks and clears a folder with everything inside it, however far down", () => {
    const choice = set(start(), at("Clients"), false);
    expect(checks(choice, ["Clients", "Clients/Acme", "Clients/Acme/Contracts", "Photos"])).toEqual({
      Clients: "off",
      "Clients/Acme": "off",
      "Clients/Acme/Contracts": "off",
      Photos: "on",
    });
    // One rule stands for all of them: nothing below Clients is listed.
    expect(choice.rules).toEqual({ [at("Clients")]: false });
    expect(isEverything(choice)).toBe(false);
  });

  it("shows a dash on every folder above one decided otherwise", () => {
    const choice = set(start(), at("Photos/2021/Holiday"), false);
    expect(checks(choice, ["Photos", "Photos/2021", "Photos/2021/Holiday", "Photos/2020", "Clients"])).toEqual({
      Photos: "mixed",
      "Photos/2021": "mixed",
      "Photos/2021/Holiday": "off",
      "Photos/2020": "on",
      Clients: "on",
    });
    // The folders with a dash are still ticked themselves.
    expect(taken(choice, ["Photos", "Photos/2021", "Photos/2021/Holiday"])).toEqual({ Photos: true, "Photos/2021": true, "Photos/2021/Holiday": false });
  });

  it("lets a rule further down say otherwise, and that one's insides follow it", () => {
    const choice = set(set(start(), at("Photos"), false), at("Photos/2021"), true);
    expect(taken(choice, ["Photos", "Photos/2020", "Photos/2021", "Photos/2021/Holiday", "Photos/2021/Holiday/Beach"])).toEqual({
      Photos: false,
      "Photos/2020": false,
      "Photos/2021": true,
      "Photos/2021/Holiday": true,
      "Photos/2021/Holiday/Beach": true,
    });
    expect(check(choice, at("Photos"))).toBe("mixed");
    expect(forRun(choice).rules).toEqual([
      { path: at("Photos"), on: false },
      { path: at("Photos/2021"), on: true },
    ]);
  });

  it("forgets the rules inside a folder that's ticked or cleared whole, and a rule that says nothing new", () => {
    let choice = set(set(start(), at("Photos/2021/Holiday"), false), at("Photos/2020"), false);
    expect(ruledPaths(choice)).toHaveLength(2);
    choice = set(choice, at("Photos"), true);
    expect(isEverything(choice)).toBe(true);
    // Ticking a folder that's ticked already changes nothing.
    expect(set(start(), at("Clients"), true).rules).toEqual({});
    // Clearing what's inside a cleared folder changes nothing either.
    const cleared = set(start(), at("Clients"), false);
    expect(set(cleared, at("Clients/Acme"), false)).toEqual(cleared);
  });

  it("ticks a folder with a dash whole when its box is clicked, and clears it the next time", () => {
    let choice = set(start(), at("Photos/2021/Holiday"), false);
    choice = toggle(choice, at("Photos"));
    expect(check(choice, at("Photos"))).toBe("on");
    expect(isEverything(choice)).toBe(true);
    choice = toggle(choice, at("Photos"));
    expect(check(choice, at("Photos"))).toBe("off");
    expect(check(choice, at("Photos/2021/Holiday"))).toBe("off");
  });

  it("selects all or none, and ticks a few out of none", () => {
    const none = setAll(set(start(), at("Clients"), false), false);
    expect(isNothing(none)).toBe(true);
    expect(forRun(none)).toEqual({ all: false, rules: [] });
    const few = set(set(none, at("Invoices/2026"), true), at("Clients/Acme"), true);
    expect(taken(few, ["Invoices", "Invoices/2026", "Invoices/2025", "Clients/Acme/Contracts", "Design"])).toEqual({
      Invoices: false,
      "Invoices/2026": true,
      "Invoices/2025": false,
      "Clients/Acme/Contracts": true,
      Design: false,
    });
    expect(check(few, at("Invoices"))).toBe("mixed");
    expect(isEverything(setAll(few, true))).toBe(true);
  });

  it("knows a folder inside another by its whole name, not the start of it", () => {
    const choice = set(start(), at("Photos"), false);
    expect(takes(choice, at("Photos 2"))).toBe(true);
    expect(check(choice, at("Photos 2"))).toBe("on");
    expect(check(set(start(), at("Photos 2/Raw"), false), at("Photos"))).toBe("on");
  });

  it("works with Windows' separator", () => {
    const choice = set(everything("C:\\Users\\me\\Projects", "\\"), "C:\\Users\\me\\Projects\\Photos", false);
    expect(pathIn(choice, "C:\\Users\\me\\Projects\\Photos", "2021")).toBe("C:\\Users\\me\\Projects\\Photos\\2021");
    expect(takes(choice, "C:\\Users\\me\\Projects\\Photos\\2021")).toBe(false);
    expect(takes(choice, "C:\\Users\\me\\Projects\\Design")).toBe(true);
  });
});

describe("how many folders a choice takes", () => {
  const found = (inside: number, done = true): PathCount => ({ found: true, inside, done });
  const counted = (choice: Choice, root: { inside: number; done: boolean }, counts: Record<string, PathCount>) =>
    countChosen(
      choice,
      root,
      ruledPaths(choice).map((p) => counts[p]),
    );

  it("is every folder inside when nothing was cleared", () => {
    expect(counted(start(), { inside: 28, done: true }, {})).toEqual({ count: 28, total: 28, done: true });
  });

  it("leaves out a cleared folder and everything inside it", () => {
    // Clients and its 4 folders inside: 5 of 28 go.
    const choice = set(start(), at("Clients"), false);
    expect(counted(choice, { inside: 28, done: true }, { [at("Clients")]: found(4) })).toEqual({ count: 23, total: 28, done: true });
  });

  it("adds back what a rule further down ticks again", () => {
    // Photos (6 inside) cleared, then 2021 (2 inside) in it ticked: 28 - 7 + 3.
    const choice = set(set(start(), at("Photos"), false), at("Photos/2021"), true);
    const counts = { [at("Photos")]: found(6), [at("Photos/2021")]: found(2) };
    expect(counted(choice, { inside: 28, done: true }, counts).count).toBe(24);
  });

  it("counts only what's ticked when nothing is by default", () => {
    const choice = set(set(setAll(start(), false), at("Invoices"), true), at("Invoices/2025"), false);
    const counts = { [at("Invoices")]: found(2), [at("Invoices/2025")]: found(0) };
    expect(counted(choice, { inside: 28, done: true }, counts)).toEqual({ count: 2, total: 28, done: true });
  });

  it("is what's been found so far until every count it needs is done", () => {
    const choice = set(start(), at("Clients"), false);
    // The count hasn't come to Clients yet: none of it is among the folders found so far.
    expect(counted(choice, { inside: 12, done: false }, { [at("Clients")]: { found: false, inside: 0, done: false } })).toEqual({
      count: 12,
      total: 12,
      done: false,
    });
    // Clients is counted, the rest isn't yet.
    expect(counted(choice, { inside: 400, done: false }, { [at("Clients")]: found(4) })).toEqual({ count: 395, total: 400, done: false });
    // A folder a rule is on that isn't there any more takes nothing away.
    expect(counted(choice, { inside: 28, done: true }, { [at("Clients")]: { found: false, inside: 0, done: true } })).toEqual({
      count: 28,
      total: 28,
      done: true,
    });
  });
});
