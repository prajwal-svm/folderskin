import { useEffect, useRef, type ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { REPO_URL } from "../lib/packs";
import type { UpdateStatus } from "../hooks/useUpdates";
import { UpdateButton } from "./UpdateDialog";
import { BadgeAlertIcon } from "./icons/badge-alert";
import { GithubMark } from "./icons/githubMark";
import { StarIcon } from "./icons/star";
import { useT } from "../i18n";

const open = (url: string) => void openUrl(url).catch(() => {});

/**
 * About FolderSkin, under the version badge beside the logo. It opens while the badge is hovered
 * or focused and stays open while the pointer is over it, so its links can be reached.
 */
export function AboutMenu({
  open: shown,
  onHover,
  onClose,
  updates,
  onCheckUpdates,
  onShowUpdate,
}: {
  open: boolean;
  /** The pointer or focus came into the popover (true) or left it (false). */
  onHover: (inside: boolean) => void;
  onClose: () => void;
  updates: UpdateStatus;
  onCheckUpdates: () => void;
  onShowUpdate: () => void;
}) {
  const t = useT();
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!shown) return;
    const close = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (ref.current && !ref.current.contains(target) && !target.closest(".brand-version")) onClose();
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [shown, onClose]);

  if (!shown) return null;
  const link = (url: string, label: string, icon: ReactNode) => (
    <button type="button" className="about-link" onClick={() => open(url)}>
      {icon}
      {label}
    </button>
  );
  return (
    <div
      className="about-pop"
      role="dialog"
      aria-label={t("sidebar.about")}
      ref={ref}
      onMouseEnter={() => onHover(true)}
      onMouseLeave={() => onHover(false)}
      onFocus={() => onHover(true)}
      onBlur={() => onHover(false)}
    >
      <div className="about-head">
        <div className="about-head-text">
          <p className="about-title">
            Folder<span className="brand-accent">Skin</span> <span className="about-version">v{__APP_VERSION__}</span>
          </p>
          <p className="about-line">{t("common.about.licence")}</p>
        </div>
        <button type="button" className="icon-btn about-source" aria-label={t("updates.about.sourceLabel")} data-tip={t("updates.about.source")} onClick={() => open(REPO_URL)}>
          <GithubMark size={17} />
        </button>
      </div>
      <p className="about-note">{t("updates.about.note")}</p>
      <div className="about-links">
        {link(`${REPO_URL}/issues`, t("common.about.reportIssues"), <BadgeAlertIcon size={15} />)}
        {link(REPO_URL, t("common.about.star"), <StarIcon size={15} className="about-star" />)}
      </div>
      <UpdateButton status={updates} onCheck={onCheckUpdates} onShow={onShowUpdate} />
    </div>
  );
}
