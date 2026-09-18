import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

const REPO_URL = "https://github.com/YOUR-USER/folderskin";

export function AboutMenu({ note }: { note: string }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  const link = (url: string, label: string) => (
    <button type="button" className="about-link" onClick={() => void openUrl(url).catch(() => {})}>
      {label}
    </button>
  );

  return (
    <div className="about" ref={ref}>
      <button
        type="button"
        className="pill-btn icon-btn"
        aria-label="about FolderSkin"
        aria-expanded={open}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => setOpen((v) => !v)}
      >
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
          <path
            d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7zm8.4-3.5l1.6-1.3-2-3.4-2 .7a7.6 7.6 0 0 0-1.6-.9L16 5h-4l-.4 2.1a7.6 7.6 0 0 0-1.6.9l-2-.7-2 3.4L7.6 12 6 13.3l2 3.4 2-.7c.5.4 1 .7 1.6.9L12 19h4l.4-2.1c.6-.2 1.1-.5 1.6-.9l2 .7 2-3.4-1.6-1.3z"
            fill="currentColor"
          />
        </svg>
      </button>
      {open && (
        <div className="about-pop" role="dialog" aria-label="about FolderSkin">
          <p className="about-title">
            <span className="brand-a">folder</span>skin <span className="about-version">{__APP_VERSION__}</span>
          </p>
          <p className="about-line">free and open source · MIT</p>
          {note && <p className="about-note">{note}</p>}
          <div className="about-links">
            {link(REPO_URL, "source code")}
            {link(`${REPO_URL}/issues`, "report a problem")}
          </div>
        </div>
      )}
    </div>
  );
}
