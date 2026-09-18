import { useEffect, useRef } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

const REPO_URL = "https://github.com/prajwal-svm/folderskin";

/**
 * About FolderSkin, under the version badge beside the logo. It opens while the badge is hovered
 * or focused and stays open while the pointer is over it, so its links can be reached.
 */
export function AboutMenu({
  note,
  open,
  onHover,
  onClose,
}: {
  note: string;
  open: boolean;
  /** The pointer or focus came into the popover (true) or left it (false). */
  onHover: (inside: boolean) => void;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      const t = e.target as HTMLElement;
      if (ref.current && !ref.current.contains(t) && !t.closest('[aria-label="about FolderSkin"]')) onClose();
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [open, onClose]);

  if (!open) return null;
  const link = (url: string, label: string) => (
    <button type="button" className="about-link" onClick={() => void openUrl(url).catch(() => {})}>
      {label}
    </button>
  );
  return (
    <div
      className="about-pop"
      role="dialog"
      aria-label="about FolderSkin"
      ref={ref}
      onMouseEnter={() => onHover(true)}
      onMouseLeave={() => onHover(false)}
      onFocus={() => onHover(true)}
      onBlur={() => onHover(false)}
    >
      <p className="about-title">
        FolderSkin <span className="about-version">v{__APP_VERSION__}</span>
      </p>
      <p className="about-line">Free and open source · MIT</p>
      {note && <p className="about-note">{note}</p>}
      <div className="about-links">
        {link(REPO_URL, "Source code")}
        {link(`${REPO_URL}/issues`, "Report a problem")}
      </div>
    </div>
  );
}
