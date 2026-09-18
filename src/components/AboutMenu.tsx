import { useEffect, useRef } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

const REPO_URL = "https://github.com/YOUR-USER/folderskin";

/** Popover anchored to the rail's About button; the rail owns the open state. */
export function AboutMenu({ note, open, onClose }: { note: string; open: boolean; onClose: () => void }) {
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
    <div className="about-pop" role="dialog" aria-label="about FolderSkin" ref={ref}>
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
