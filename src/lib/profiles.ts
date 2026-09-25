/**
 * Licence profiles: who a shared pack is credited to and the licence it goes out under, kept
 * together under a name so each way someone shares ("Personal", "For work") is one choice. One
 * profile is the default, the one sharing starts from. Kept on this computer.
 */
import { isGithubUser, LICENSES } from "./licences";

export type LicenseId = (typeof LICENSES)[number]["id"];

export type LicenceProfile = {
  id: string;
  /** What the profile is called in Settings and the share dialog. */
  name: string;
  /** The name packs are credited to: letters, digits and single dashes. Empty until one is given. */
  author: string;
  license: LicenseId;
};

export type Profiles = { list: LicenceProfile[]; defaultId: string };

export const PROFILES_KEY = "folderskin.sharing.profiles";
/** Where sharing kept its one author and licence before there were profiles. */
const OLD_SHARING_KEY = "folderskin.sharing";

export const MAX_PROFILES = 12;
export const MAX_PROFILE_NAME = 40;

const isLicense = (id: unknown): id is LicenseId => LICENSES.some((l) => l.id === id);

export function newProfileId(): string {
  return `p${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
}

/** The first profile anyone has: what sharing remembered before, or nothing yet. */
function firstProfiles(old: unknown): Profiles {
  const o = typeof old === "object" && old !== null ? (old as Record<string, unknown>) : {};
  const profile: LicenceProfile = {
    id: "personal",
    name: "Personal",
    author: typeof o.author === "string" && isGithubUser(o.author) ? o.author : "",
    license: isLicense(o.license) ? o.license : LICENSES[0].id,
  };
  return { list: [profile], defaultId: profile.id };
}

/**
 * Profiles as saved, made sound: unknown licences back to the first, names trimmed, repeated ids
 * dropped, a name used twice numbered ("Profile 2"), and a default that is one of them. Never
 * empty.
 */
export function readProfiles(raw: unknown, old?: unknown): Profiles {
  const v = typeof raw === "object" && raw !== null ? (raw as Record<string, unknown>) : null;
  if (!v || !Array.isArray(v.list)) return firstProfiles(old);
  const seen = new Set<string>();
  const names = new Set<string>();
  const list: LicenceProfile[] = [];
  for (const p of v.list.slice(0, MAX_PROFILES)) {
    if (typeof p !== "object" || p === null) continue;
    const r = p as Record<string, unknown>;
    if (typeof r.id !== "string" || !r.id || seen.has(r.id)) continue;
    seen.add(r.id);
    const name = unusedName(cleanProfileName(typeof r.name === "string" ? r.name : "") || "Profile", names);
    names.add(name.toLowerCase());
    list.push({
      id: r.id,
      name,
      author: typeof r.author === "string" && isGithubUser(r.author) ? r.author : "",
      license: isLicense(r.license) ? r.license : LICENSES[0].id,
    });
  }
  if (list.length === 0) return firstProfiles(old);
  const defaultId = list.some((p) => p.id === v.defaultId) ? (v.defaultId as string) : list[0].id;
  return { list, defaultId };
}

export function cleanProfileName(name: string): string {
  return Array.from(name.replace(/\s+/g, " ").trim()).slice(0, MAX_PROFILE_NAME).join("");
}

/** `name`, or with the first number after it that no name in `taken` (lowercased) has yet. */
function unusedName(name: string, taken: Set<string>): string {
  let next = name;
  for (let n = 2; taken.has(next.toLowerCase()); n++) {
    const suffix = ` ${n}`;
    next = Array.from(name).slice(0, MAX_PROFILE_NAME - suffix.length).join("").trimEnd() + suffix;
  }
  return next;
}

/** One key as JSON, or null when it's missing, broken or can't be read. */
function readKey(key: string): unknown {
  try {
    return JSON.parse(localStorage.getItem(key) ?? "null");
  } catch {
    return null;
  }
}

export function loadProfiles(): Profiles {
  // Read apart, so broken profiles still fall back to what sharing kept before them.
  const raw = readKey(PROFILES_KEY);
  return readProfiles(raw, raw ? undefined : readKey(OLD_SHARING_KEY));
}

export function saveProfiles(profiles: Profiles): void {
  try {
    localStorage.setItem(PROFILES_KEY, JSON.stringify(profiles));
  } catch {
    /* nowhere to keep them: they last until the app closes */
  }
}

export const defaultProfile = (p: Profiles): LicenceProfile => p.list.find((x) => x.id === p.defaultId) ?? p.list[0];

/** What's wrong with a profile, and the field it's about ("list" when it's no one field). */
export type ProfileProblem = { field: "name" | "author" | "list"; text: string };

/**
 * Why a profile can't be kept as it is, or null when it can. `others` is every profile but this
 * one, so a new profile finds the list already full there.
 */
export function profileProblem(p: Pick<LicenceProfile, "name" | "author">, others: LicenceProfile[]): ProfileProblem | null {
  const name = cleanProfileName(p.name);
  if (!name) return { field: "name", text: "Give the profile a name." };
  if (others.some((o) => o.name.toLowerCase() === name.toLowerCase())) return { field: "name", text: `There's a profile called ${name} already.` };
  if (p.author && !isGithubUser(p.author)) return { field: "author", text: "Credit goes to a name made of letters, numbers and single dashes." };
  if (others.length >= MAX_PROFILES) return { field: "list", text: `There can be ${MAX_PROFILES} profiles at most. Delete one to add this one.` };
  return null;
}

export function upsertProfile(profiles: Profiles, profile: LicenceProfile): Profiles {
  const clean = { ...profile, name: cleanProfileName(profile.name) };
  const at = profiles.list.findIndex((p) => p.id === profile.id);
  if (at === -1) {
    if (profiles.list.length >= MAX_PROFILES) return profiles;
    return { ...profiles, list: [...profiles.list, clean] };
  }
  const list = profiles.list.slice();
  list[at] = clean;
  return { ...profiles, list };
}

/** Removes a profile. The last one stays; the default passes to the first left. */
export function removeProfile(profiles: Profiles, id: string): Profiles {
  if (profiles.list.length <= 1) return profiles;
  const list = profiles.list.filter((p) => p.id !== id);
  if (list.length === profiles.list.length) return profiles;
  const defaultId = profiles.defaultId === id ? list[0].id : profiles.defaultId;
  return { list, defaultId };
}

export function makeDefault(profiles: Profiles, id: string): Profiles {
  return profiles.list.some((p) => p.id === id) ? { ...profiles, defaultId: id } : profiles;
}
