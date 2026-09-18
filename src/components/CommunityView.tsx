import { openUrl } from "@tauri-apps/plugin-opener";
import { DownloadIcon } from "./icons/download";
import { ExternalLinkIcon } from "./icons/external-link";
import { ImageIcon } from "./icons/image";

export const COMMUNITY_URL = "https://github.com/prajwal-svm/folderskin/discussions/categories/skins";

export function CommunityView({ onImport }: { onImport: () => void }) {
  return (
    <section className="view">
      <header className="view-head">
        <h2 className="view-title">
          Community skins
        </h2>
        <p className="view-sub">Skins other people made, and a place to share yours.</p>
      </header>
      <div className="cards">
        <article className="card">
          <span className="card-glyph">
            <ImageIcon size={20} />
          </span>
          <h3 className="card-title">Load a skin from a file</h3>
          <p className="card-text">PNG, JPEG, WebP or HEIC. It lands in Yours and stays there. Finished folders on a magenta background are cut out automatically.</p>
          <button type="button" className="btn btn-primary" onMouseDown={(e) => e.preventDefault()} onClick={onImport}>
            <DownloadIcon size={15} />
            Choose a picture
          </button>
        </article>
        <article className="card">
          <span className="card-glyph">
            <ExternalLinkIcon size={20} />
          </span>
          <h3 className="card-title">Browse shared skins</h3>
          <p className="card-text">Community skins live in the project's discussions. Download one, then load it here.</p>
          <button
            type="button"
            className="btn btn-secondary"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => void openUrl(COMMUNITY_URL).catch(() => {})}
          >
            <ExternalLinkIcon />
            Open community skins
          </button>
        </article>
        <article className="card card-soon">
          <span className="chip">Coming next</span>
          <h3 className="card-title">One-click skin packs</h3>
          <p className="card-text">Install and update community packs from inside the app, and publish your own.</p>
        </article>
      </div>
    </section>
  );
}
