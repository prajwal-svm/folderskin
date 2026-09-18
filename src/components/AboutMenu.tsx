import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { IconSliders } from "./icons";

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
        className="icon-btn"
        aria-label="about FolderSkin"
        aria-expanded={open}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => setOpen((v) => !v)}
      >
        <IconSliders />
      </button>
      {open && (
        <div className="about-pop" role="dialog" aria-label="about FolderSkin">
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
      )}
    </div>
  );
}
