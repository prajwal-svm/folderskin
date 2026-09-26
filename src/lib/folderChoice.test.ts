import { describe, expect, it } from "vitest";
import { FolderChoice, type SubfolderTree } from "./folderChoice";

/**
 * A tree the way `subfolder_tree` sends it, from `/`-separated paths inside `/work/Projects`,
 * nearest first as the walk finds them.
 */
function treeOf(paths: string[], more = false): SubfolderTree {
  const sorted = [...paths].sort((a, b) => a.split("/").length - b.split("/").length);
  const index = new Map<string, number>([["", 0]]);
  const names: string[] = [];
  const parents: number[] = [];
  sorted.forEach((path, i) => {
    const cut = path.lastIndexOf("/");
    parents.push(index.get(cut < 0 ? "" : path.slice(0, cut))!);
    names.push(path.slice(cut + 1));
    index.set(path, i + 1);
  });
  return { root: "/work/Projects", separator: "/", names, parents, more };
}

const PROJECTS = [
  "Clients",
  "Design",
  "Photos",
  "Clients/Acme",
  "Clients/Globex",
  "Photos/2021",
  "Photos/2020",
  "Clients/Acme/Contracts",
  "Photos/2021/Holiday",
];

/** A folder's number by its path. */
function nodeAt(choice: FolderChoice, path: string): number {
  const full = `/work/Projects/${path}`;
  for (let k = 1; k <= choice.total; k++) if (choice.path(k) === full) return k;
  throw new Error(`no ${path}`);
}

/** Each named folder's box. */
const checks = (choice: FolderChoice, paths: string[]) => Object.fromEntries(paths.map((p) => [p, choice.check(nodeAt(choice, p))]));

describe("choosing folders inside a folder", () => {
  it("starts with every folder ticked, as including subfolders takes them all", () => {
    const choice = new FolderChoice(treeOf(PROJECTS));
    expect(choice.total).toBe(9);
    expect(choice.chosen).toBe(9);
    expect(checks(choice, ["Clients", "Photos/2021/Holiday"])).toEqual({ Clients: "on", "Photos/2021/Holiday": "on" });
    expect(choice.name(0)).toBe("Projects");
    expect(choice.isChosen(0)).toBe(true);
  });

  it("ticks and clears a folder with everything inside it", () => {
    const choice = new FolderChoice(treeOf(PROJECTS));
    const clients = nodeAt(choice, "Clients");
    expect(choice.set(clients, false)).toBe(true);
    expect(checks(choice, ["Clients", "Clients/Acme", "Clients/Globex", "Clients/Acme/Contracts", "Photos"])).toEqual({
      Clients: "off",
      "Clients/Acme": "off",
      "Clients/Globex": "off",
      "Clients/Acme/Contracts": "off",
      Photos: "on",
    });
    expect(choice.chosen).toBe(5);
    expect(choice.set(clients, false)).toBe(false);
    choice.set(clients, true);
    expect(choice.chosen).toBe(9);
    expect(choice.check(nodeAt(choice, "Clients/Acme/Contracts"))).toBe("on");
  });

  it("shows a dash on every folder some of whose folders are ticked", () => {
    const choice = new FolderChoice(treeOf(PROJECTS));
    choice.set(nodeAt(choice, "Photos/2021/Holiday"), false);
    expect(checks(choice, ["Photos", "Photos/2021", "Photos/2021/Holiday", "Photos/2020", "Clients"])).toEqual({
      Photos: "mixed",
      "Photos/2021": "mixed",
      "Photos/2021/Holiday": "off",
      "Photos/2020": "on",
      Clients: "on",
    });
    // The folders themselves still take part: only Holiday was cleared.
    expect(choice.isChosen(nodeAt(choice, "Photos/2021"))).toBe(true);
    expect(choice.chosen).toBe(8);

    // Cleared from the top, then one folder deep inside ticked again: the folders around it keep
    // their dash, and aren't chosen themselves.
    choice.set(nodeAt(choice, "Photos"), false);
    choice.set(nodeAt(choice, "Photos/2021/Holiday"), true);
    expect(checks(choice, ["Photos", "Photos/2021", "Photos/2020"])).toEqual({ Photos: "mixed", "Photos/2021": "mixed", "Photos/2020": "off" });
    expect(choice.isChosen(nodeAt(choice, "Photos"))).toBe(false);
    expect(choice.chosen).toBe(6);
  });

  it("ticks everything inside a folder with a dash when its box is clicked, and clears a ticked one", () => {
    const choice = new FolderChoice(treeOf(PROJECTS));
    const photos = nodeAt(choice, "Photos");
    choice.set(nodeAt(choice, "Photos/2020"), false);
    expect(choice.check(photos)).toBe("mixed");
    choice.toggle(photos);
    expect(choice.check(photos)).toBe("on");
    expect(choice.chosen).toBe(9);
    choice.toggle(photos);
    expect(choice.check(photos)).toBe("off");
    expect(choice.chosen).toBe(5);
    choice.toggle(photos);
    expect(choice.check(photos)).toBe("on");
  });

  it("keeps a folder's own tick apart from the folders inside it", () => {
    const choice = new FolderChoice(treeOf(PROJECTS));
    const photos = nodeAt(choice, "Photos");
    choice.set(nodeAt(choice, "Photos/2021"), false);
    choice.set(nodeAt(choice, "Photos/2020"), false);
    // Only Photos itself is left: a dash, since the folders inside it aren't.
    expect(choice.check(photos)).toBe("mixed");
    expect(choice.isChosen(photos)).toBe(true);
    expect(choice.inside(photos)).toBe(3);
    expect(choice.chosenInside(photos)).toBe(0);
  });

  it("ticks or clears the lot at once", () => {
    const choice = new FolderChoice(treeOf(PROJECTS));
    choice.setAll(false);
    expect(choice.chosen).toBe(0);
    expect(choice.check(nodeAt(choice, "Clients"))).toBe("off");
    expect(choice.isChosen(0)).toBe(true);
    choice.set(nodeAt(choice, "Design"), true);
    expect(choice.chosen).toBe(1);
    choice.setAll(true);
    expect(choice.chosen).toBe(9);
    expect(choice.check(nodeAt(choice, "Photos/2021"))).toBe("on");
  });

  it("names the ticked folders nearest first, as a run takes them", () => {
    const choice = new FolderChoice(treeOf(PROJECTS));
    choice.set(nodeAt(choice, "Clients/Acme"), false);
    choice.set(nodeAt(choice, "Photos/2020"), false);
    expect(choice.chosenPaths()).toEqual([
      "/work/Projects/Clients",
      "/work/Projects/Design",
      "/work/Projects/Photos",
      "/work/Projects/Clients/Globex",
      "/work/Projects/Photos/2021",
      "/work/Projects/Photos/2021/Holiday",
    ]);
    expect(choice.path(0)).toBe("/work/Projects");
  });

  it("ticks again exactly the folders chosen before", () => {
    const first = new FolderChoice(treeOf(PROJECTS));
    first.set(nodeAt(first, "Clients"), false);
    first.set(nodeAt(first, "Photos/2021/Holiday"), false);
    const kept = first.chosenPaths();

    // Read again, with a folder made since: it isn't ticked, as nobody chose it.
    const again = new FolderChoice(treeOf([...PROJECTS, "Design/Icons"]));
    again.pick(kept);
    expect(again.chosenPaths()).toEqual(kept);
    expect(again.chosen).toBe(kept.length);
    expect(checks(again, ["Clients", "Photos/2021", "Design", "Design/Icons"])).toEqual({
      Clients: "off",
      "Photos/2021": "mixed",
      Design: "mixed",
      "Design/Icons": "off",
    });
  });

  it("lists each folder's folders the way Finder does", () => {
    const choice = new FolderChoice(treeOf(["Photos", "Photos/Photos 10", "Photos/photos 2", "Photos/Photos 1", "Photos/archive", "Photos/Zebra", "Photos/Éclair"]));
    const photos = nodeAt(choice, "Photos");
    const names = [...choice.children(photos)].map((k) => choice.name(k));
    expect(names).toEqual(["archive", "Éclair", "Photos 1", "photos 2", "Photos 10", "Zebra"]);
    names.forEach((name, i) => expect(choice.indexOf(nodeAt(choice, `Photos/${name}`))).toBe(i));
    expect(choice.childCount(photos)).toBe(6);
    expect(choice.hasChildren(nodeAt(choice, "Photos/Zebra"))).toBe(false);
  });

  it("knows where each folder is", () => {
    const choice = new FolderChoice(treeOf(PROJECTS));
    const contracts = nodeAt(choice, "Clients/Acme/Contracts");
    expect(choice.depth(contracts)).toBe(3);
    expect(choice.chain(contracts).map((k) => choice.name(k))).toEqual(["Clients", "Acme", "Contracts"]);
    expect(choice.parentOf(nodeAt(choice, "Clients"))).toBe(0);
    expect(choice.parentOf(0)).toBe(-1);
    expect(choice.chain(0)).toEqual([]);
  });

  it("leaves the root alone: it always takes part", () => {
    const choice = new FolderChoice(treeOf(PROJECTS));
    expect(choice.set(0, false)).toBe(false);
    expect(choice.isChosen(0)).toBe(true);
    expect(choice.chosen).toBe(9);
  });

  it("builds a Windows path with Windows' separator", () => {
    const choice = new FolderChoice({ root: "C:\\Users\\me\\Projects", separator: "\\", names: ["Clients", "Acme"], parents: [0, 1], more: false });
    expect(choice.chosenPaths()).toEqual(["C:\\Users\\me\\Projects\\Clients", "C:\\Users\\me\\Projects\\Clients\\Acme"]);
    expect(choice.name(0)).toBe("Projects");
  });

  it("refuses a tree whose folders come before the folder they're in", () => {
    expect(() => new FolderChoice({ root: "/r", separator: "/", names: ["a", "b"], parents: [2, 0], more: false })).toThrow();
    expect(() => new FolderChoice({ root: "/r", separator: "/", names: ["a"], parents: [0, 0], more: false })).toThrow();
    expect(new FolderChoice({ root: "/r", separator: "/", names: [], parents: [], more: false }).total).toBe(0);
  });
});

/**
 * Nearly as big as a run takes: 1,200 folders in one, a branch 34 levels deep, and wide, bushy
 * folders around them, 4,968 folders in all.
 */
function bigTree(): SubfolderTree {
  const paths: string[] = [];
  for (let i = 1; i <= 1200; i++) paths.push(`Camera roll/Day ${i}`);
  let deep = "Deep";
  for (let i = 1; i <= 33; i++) paths.push((deep = `${deep}/Level ${i}`));
  for (let c = 1; c <= 40; c++) for (let p = 1; p <= 3; p++) for (const part of ["Brief", "Drafts", "Final", "Invoices"]) paths.push(`Clients/Client ${c}/Project ${p}/${part}`);
  for (let a = 1; a <= 4; a++) for (let b = 1; b <= 4; b++) for (let c = 1; c <= 4; c++) for (let d = 1; d <= 4; d++) paths.push(`Research/${a}/${b}/${c}/${d}`);
  // Every folder on the way to each one, once.
  const all = new Set<string>();
  for (const path of paths) {
    const parts = path.split("/");
    for (let i = 1; i <= parts.length; i++) all.add(parts.slice(0, i).join("/"));
  }
  all.add("Music");
  for (let m = 1; all.size < 4968; m++) all.add(`Music/Artist ${m}`);
  return treeOf([...all]);
}

/**
 * Milliseconds the fastest of three runs of `run` takes. The slower ones were held up by something
 * else, such as the machine being busy or the engine still compiling the code, so the fastest is
 * the one that says how fast the code is.
 */
function fastest(run: (attempt: number) => void): number {
  let best = Number.POSITIVE_INFINITY;
  for (let attempt = 0; attempt < 3; attempt++) {
    const started = performance.now();
    run(attempt);
    best = Math.min(best, performance.now() - started);
  }
  return best;
}

describe("a tree as big as a run takes", () => {
  it("stays fast however wide or deep it is", () => {
    const tree = bigTree();
    expect(tree.names.length).toBeGreaterThan(4900);
    expect(tree.names.length).toBeLessThanOrEqual(5000);

    const choices: FolderChoice[] = [];
    const built = fastest(() => choices.push(new FolderChoice(tree)));
    const choice = choices[0];
    const camera = nodeAt(choice, "Camera roll");
    const bottom = nodeAt(choice, `Deep/${Array.from({ length: 33 }, (_, i) => `Level ${i + 1}`).join("/")}`);
    expect(choice.childCount(camera)).toBe(1200);
    expect(choice.depth(bottom)).toBe(34);

    // The widest folder, listed the way Finder lists it: sorted once for each tree.
    const sorted = fastest((attempt) => choices[attempt].children(camera));
    const shown = [...choice.children(camera)].map((k) => choice.name(k));
    expect(shown.slice(0, 3)).toEqual(["Day 1", "Day 2", "Day 3"]);
    expect(shown[shown.length - 1]).toBe("Day 1200");

    // A thousand clicks on the biggest folders and the deepest one, which end as they started.
    const clients = nodeAt(choice, "Clients");
    const deep = nodeAt(choice, "Deep");
    const clicked = fastest(() => {
      for (let i = 0; i < 250; i++) {
        choice.toggle(camera);
        choice.toggle(clients);
        choice.toggle(bottom);
        choice.toggle(deep);
      }
    });
    expect(choice.chosen).toBe(choice.total);

    choice.set(bottom, false);
    expect(choice.check(nodeAt(choice, "Deep"))).toBe("mixed");
    expect(choice.check(nodeAt(choice, "Deep/Level 1/Level 2/Level 3"))).toBe("mixed");
    expect(choice.chosen).toBe(choice.total - 1);

    let paths: string[] = [];
    const named = fastest(() => {
      paths = choice.chosenPaths();
    });
    expect(paths).toHaveLength(choice.total - 1);
    const picked = fastest(() => {
      choice.setAll(false);
      choice.pick(paths);
    });
    expect(choice.chosen).toBe(choice.total - 1);
    const cleared = fastest(() => {
      for (let i = 0; i < 100; i++) choice.setAll(i % 2 === 0);
    });

    // On a laptop: building 4 ms, sorting 1, the thousand clicks 1, naming 1, picking 2 and a hundred
    // tick-or-clear-alls under 1. The budgets leave a slow or busy CI runner twenty times that.
    expect(built).toBeLessThan(100);
    expect(sorted).toBeLessThan(100);
    expect(clicked).toBeLessThan(50);
    expect(named).toBeLessThan(50);
    expect(picked).toBeLessThan(50);
    expect(cleared).toBeLessThan(25);
  });
});
