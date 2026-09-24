import { useLayoutEffect, useMemo, useRef } from "react";
import { ctx2d, makeCanvas, type Assets } from "../../composer/assets";
import { drawFolderView, drawFreeView, type TemplateImages } from "../../composer/composite";
import { emptyDoc, fallbackParts, type Doc, type FolderStyle, type Parts, type Shape } from "../../composer/doc";
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

const EMPTY: { shape: Shape; label: string; note: string }[] = [
  { shape: "folder", label: "Empty folder", note: "Cut to the folder" },
  { shape: "free", label: "Free icon", note: "Any shape you like" },
];

/**
 * Where every new design starts: empty, or from one of the templates. A real dialog over the
 * whole window, so the design in progress can't be edited, or mistaken for the new one, while
 * it's open. When that design has changes that aren't saved, the dialog says so first and offers
 * to save it; choosing a start then replaces it, with no second "are you sure?".
 */
export function NewDesign({
  folders,
  style,
  assets,
  version,
  dirty,
  saving,
  onStart,
  onSaveFirst,
  onClose,
}: {
  /** Each folder's template layers and parts, once loaded (null when they couldn't be). */
  folders: Partial<Record<FolderStyle, { images: TemplateImages; parts: Parts } | null>>;
  /** The folder the design in progress is on, which new designs start on. */
  style: FolderStyle;
  assets: Assets;
  version: number;
  /** The design in progress has changes that aren't saved. */
  dirty: boolean;
  saving: boolean;
  onStart: (start: Start) => void;
  onSaveFirst: () => void;
  onClose: () => void;
}) {
  const templates = useMemo(
    () =>
      TEMPLATES.map((t) => {
        // A folder's own look is shown on that folder, the rest on the one the design is on.
        const on = t.style ?? style;
        const folder = folders[on];
        return { t, doc: { ...t.make(folder?.parts ?? fallbackParts(on)), style: on }, template: folder?.images ?? null };
      }),
    [folders, style],
  );
  const empty = useMemo(() => ({ folder: emptyDoc("folder", style), free: emptyDoc("free", style) }), [style]);
  // The Mac's and Windows' own folders have nothing on them yet: they start empty, like a blank folder.
  const plain = templates.filter(({ t }) => t.plain);
  const designed = templates.filter(({ t }) => !t.plain);
  const template = folders[style]?.images ?? null;

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
                <DesignThumb doc={empty[e.shape]} template={template} assets={assets} size={104} version={version} />
              </span>
              <span className="cmp-card-label">{e.label}</span>
              <span className="cmp-card-note">{e.note}</span>
            </button>
          ))}
          {plain.map(({ t, doc, template: folder }) => (
            <button key={t.id} type="button" className="cmp-card" onClick={() => onStart({ kind: "template", template: t })}>
              <span className="cmp-card-art" aria-hidden="true">
                <DesignThumb doc={doc} template={folder} assets={assets} size={104} version={version} />
              </span>
              <span className="cmp-card-label">{t.label}</span>
              <span className="cmp-card-note">{t.plain?.note}</span>
            </button>
          ))}
        </div>
      </section>
      <section className="new-section" aria-label="start from a template">
        <h3 className="new-heading">Or start from a template</h3>
        <div className="cmp-sheet-grid">
          {designed.map(({ t, doc, template: folder }, i) => (
            <button key={t.id} type="button" className="cmp-card" style={{ animationDelay: `${Math.min(i, 14) * 18}ms` }} onClick={() => onStart({ kind: "template", template: t })}>
              <span className="cmp-card-art">
                <DesignThumb doc={doc} template={folder} assets={assets} size={104} version={version} />
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
