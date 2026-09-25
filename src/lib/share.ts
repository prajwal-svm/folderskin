/**
 * Sharing a pack: the words and rules the share dialog uses for it. The Rust side is
 * src-tauri/src/share.rs, and the service it talks to is services/community.
 */
import type { MySubmission, ScaledPicture, ShareProgress } from "./tauri";
import { t } from "../i18n";
import { formatList } from "../i18n/format";

/**
 * Where the pictures came from, as the dialog asks it (`share.sources.<id>`). The service keeps the
 * answer for the review.
 */
export const PICTURE_SOURCES = [{ id: "own" }, { id: "ai" }, { id: "mixed" }, { id: "licensed" }] as const;

export type PictureSource = (typeof PICTURE_SOURCES)[number]["id"];

/**
 * A handle: the name packs shared from this computer are credited to. It has the shape of a GitHub
 * user name (letters, digits and single dashes, not at either end), because a pack's author needs
 * that, and is 3 to 39 characters. `folderskin_share::is_handle` is the same rule.
 */
export function isHandle(name: string): boolean {
  return name.length >= 3 && name.length <= 39 && /^[A-Za-z0-9]+(-[A-Za-z0-9]+)*$/.test(name);
}

/** What someone types, made into a handle's shape as they type: spaces and symbols become dashes. */
export function handleFrom(text: string): string {
  return text
    .normalize("NFKD")
    .replace(/\p{M}/gu, "")
    .replace(/[^A-Za-z0-9]+/g, "-")
    .replace(/^-+/, "")
    .slice(0, 39);
}

/** How far sending has got, in the words the dialog says while it happens. */
export function shareProgressLabel(p: ShareProgress): string {
  switch (p.stage) {
    case "preparing":
      return t("share.progress.preparing");
    case "encoding":
      return t("share.progress.encoding", { done: p.done, total: p.total });
    case "checking":
      return t("share.progress.checking");
    case "uploading":
      return t("share.progress.uploading", { done: Math.min(p.done + 1, p.total), total: p.total });
    case "finishing":
      return t("share.progress.finishing");
    case "waiting":
      return p.seconds > 1 ? t("share.progress.retryIn", { count: p.seconds }) : t("share.progress.retry");
  }
}

/** The most one of a pack's pictures can be: the service and `packs check` hold it to the same. */
export const MAX_PICTURE_MB = 1.5;

/**
 * Why some of a pack's pictures are smaller than the rest, or null when none are. A pack keeps
 * every pixel of every picture, so one too detailed to fit {@link MAX_PICTURE_MB} MB at 1024 px is
 * made 896 px, then 768 px, instead. Names a few and counts the rest.
 */
export function scaledNote(scaled: ScaledPicture[]): string | null {
  if (scaled.length === 0) return null;
  const shown = scaled.slice(0, 3).map((s) => t("share.scaled.picture", { name: s.name, side: s.side }));
  const more = scaled.length - shown.length;
  const names = formatList(more > 0 ? [...shown, t("share.scaled.more", { count: more })] : shown);
  return t("share.scaled.note", { count: scaled.length, names, max: MAX_PICTURE_MB });
}

/** A submission's status as the author reads it, and the colour its chip takes. */
export function statusLabel(status: MySubmission["status"]): { label: string; tone: "ok" | "danger" | "accent" | "plain" } {
  switch (status) {
    case "uploading":
      return { label: t("share.status.uploading"), tone: "plain" };
    case "in_review":
      return { label: t("share.status.inReview"), tone: "accent" };
    case "approved":
      return { label: t("share.status.approved"), tone: "ok" };
    case "rejected":
      return { label: t("share.status.rejected"), tone: "danger" };
    case "withdrawn":
      return { label: t("share.status.withdrawn"), tone: "plain" };
    case "taken_down":
      return { label: t("share.status.takenDown"), tone: "danger" };
    case "expired":
      return { label: t("share.status.expired"), tone: "plain" };
  }
}

/** Whether the author can still take a submission back. */
export const canWithdraw = (status: MySubmission["status"]) => status === "uploading" || status === "in_review" || status === "approved";

const HANDLE_KEY = "folderskin.sharing.handle";

/** The handle typed last time, so a second pack doesn't ask for it again before verifying. */
export function loadHandle(): string {
  try {
    const saved = localStorage.getItem(HANDLE_KEY) ?? "";
    return isHandle(saved) ? saved : "";
  } catch {
    return "";
  }
}

export function saveHandle(handle: string): void {
  try {
    localStorage.setItem(HANDLE_KEY, handle);
  } catch {
    /* nowhere to keep it: it's asked for again next time */
  }
}
