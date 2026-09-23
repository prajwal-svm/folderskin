import { useLayoutEffect, useMemo, useRef } from "react";
import { ctx2d, makeCanvas, type Assets } from "../../composer/assets";
import { drawFolderView, drawFreeView, type TemplateImages } from "../../composer/composite";
import { emptyDoc, type Doc, type Parts, type Shape } from "../../composer/doc";
import { renderDoc } from "../../composer/render";
import { TEMPLATES, type Template } from "../../composer/templates";
import { Modal } from "../Modal";
import { ImageIcon } from "../icons/image";
import { LoaderIcon } from "../icons/loader";

/** A design drawn small: on the folder, or as it is for a free icon. */
export function DesignThumb({ doc, template, assets, size, version = 0 }: { doc: Doc; template: TemplateImages | null; assets: Assets; size: number; version?: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useLayoutEffect(() => {
    const c = ref.current;
    if (!c) return;
    const px = Math.round(size * Math.min(2, window.devicePixelRatio || 1));
    c.width = px;
    c.height = px;
    const design = makeCanvas(px, px);
    renderDoc(ctx2d(design), doc, px, assets);
    const ctx = ctx2d(c);
    ctx.clearRect(0, 0, px, px);
    // As the canvas shows it: what's see-through shows the folder's shape, or the icon's square.
    if (template && doc.shape === "folder") drawFolderView(ctx, design, template, px, makeCanvas(px, px));
    else drawFreeView(ctx, design, px);
  }, [doc, template, assets, size, version]);
  return <canvas ref={ref} className="cmp-design-thumb" style={{ width: size, height: size }} aria-hidden="true" />;
}

/** How a new design starts: empty, as a folder or a free icon, or from a template. */
export type Start = { kind: "empty"; shape: Shape } | { kind: "template"; template: Template };

const EMPTY_FOLDER = emptyDoc("folder");
const EMPTY_FREE = emptyDoc("free");

const EMPTY: { shape: Shape; label: string; note: string }[] = [
  { shape: "folder", label: "Empty folder", note: "Cut to FolderSkin's folder" },
  { shape: "free", label: "Free icon", note: "Any shape you like" },
];

/**
 * Where every new design starts: empty, or from one of the templates. A real dialog over the
 * whole window, so the design in progress can't be edited, or mistaken for the new one, while
 * it's open. When that design has changes that aren't saved, the dialog says so first and offers
 * to save it; choosing a start then replaces it, with no second "are you sure?".
 */
export function NewDesign({
  parts,
  template,
  assets,
  version,
  dirty,
  saving,
  onStart,
  onSaveFirst,
  onClose,
}: {
  parts: Parts;
  template: TemplateImages | null;
  assets: Assets;
  version: number;
  /** The design in progress has changes that aren't saved. */
  dirty: boolean;
  saving: boolean;
  onStart: (start: Start) => void;
  onSaveFirst: () => void;
  onClose: () => void;
}) {
  const templates = useMemo(() => TEMPLATES.map((t) => ({ t, doc: t.make(parts) })), [parts]);

  return (
    <Modal
      title="Start a new design"
      sub="Start empty, or from a template. Everything on it can be changed: colours, words, icons, pictures, even the folder's shape."
      className="modal-new"
      onClose={onClose}
    >
      {dirty && (
        <div className="new-unsaved" role="note">
          <p className="new-unsaved-text">
            <strong>Your current design isn't saved.</strong> Starting a new one discards its changes.
          </p>
          <button type="button" className="btn btn-secondary" onClick={onSaveFirst} disabled={saving} data-modal-focus>
            {saving && <LoaderIcon size={15} />}
            Save it first
          </button>
        </div>
      )}
      <section className="new-section" aria-label="start empty">
        <h3 className="new-heading">Start empty</h3>
        <div className="cmp-sheet-grid">
          {EMPTY.map((e) => (
            <button key={e.shape} type="button" className="cmp-card is-empty" onClick={() => onStart({ kind: "empty", shape: e.shape })}>
              {/* Drawn as the canvas will show it: the folder, or the square, with nothing on it yet. */}
              <span className="cmp-card-art" aria-hidden="true">
                <DesignThumb doc={e.shape === "folder" ? EMPTY_FOLDER : EMPTY_FREE} template={template} assets={assets} size={104} version={version} />
              </span>
              <span className="cmp-card-label">{e.label}</span>
              <span className="cmp-card-note">{e.note}</span>
            </button>
          ))}
        </div>
      </section>
      <section className="new-section" aria-label="start from a template">
        <h3 className="new-heading">Or start from a template</h3>
        <div className="cmp-sheet-grid">
          {templates.map(({ t, doc }, i) => (
            <button key={t.id} type="button" className="cmp-card" style={{ animationDelay: `${Math.min(i, 14) * 18}ms` }} onClick={() => onStart({ kind: "template", template: t })}>
              <span className="cmp-card-art">
                <DesignThumb doc={doc} template={template} assets={assets} size={104} version={version} />
                {t.photo && (
                  <span className="cmp-card-badge" aria-hidden="true" data-tip="Asks for a picture">
                    <ImageIcon size={13} />
                  </span>
                )}
              </span>
              <span className="cmp-card-label">{t.label}</span>
            </button>
          ))}
        </div>
      </section>
    </Modal>
  );
}
