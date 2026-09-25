import "../../i18n/composer";
import { useT, type MessageKey } from "../../i18n";
import { Rich } from "../../i18n/Rich";
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

/** The two empty starts; each is named by `composer.new.empty.<shape>` with a note under it. */
const EMPTY: Shape[] = ["folder", "free"];

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
  const t = useT();
  const templates = useMemo(
    () =>
      TEMPLATES.map((tpl) => {
        // A folder's own look is shown on that folder, the rest on the one the design is on.
        const on = tpl.style ?? style;
        const folder = folders[on];
        return { tpl, doc: { ...tpl.make(folder?.parts ?? fallbackParts(on)), style: on }, template: folder?.images ?? null };
      }),
    // Made again in a new language: the templates' words are the language's.
    [folders, style, t],
  );
  const empty = useMemo(() => ({ folder: emptyDoc("folder", style), free: emptyDoc("free", style) }), [style]);
  // The Mac's and Windows' own folders have nothing on them yet: they start empty, like a blank folder.
  const plain = templates.filter(({ tpl }) => tpl.plain);
  const designed = templates.filter(({ tpl }) => !tpl.plain);
  const template = folders[style]?.images ?? null;

  return (
    <Modal
      title={t("composer.new.title")}
      sub={t("composer.new.sub")}
      className="modal-new"
      onClose={onClose}
    >
      {dirty && (
        <div className="new-unsaved" role="note">
          <p className="new-unsaved-text">
            <Rich k="composer.new.unsaved" tags={{ b: (s) => <strong>{s}</strong> }} />
          </p>
          <button type="button" className="btn btn-secondary" onClick={onSaveFirst} disabled={saving} data-modal-focus>
            {saving && <LoaderIcon size={15} />}
            {t("composer.new.saveFirst")}
          </button>
        </div>
      )}
      <section className="new-section" aria-label={t("composer.new.emptyLabel")}>
        <h3 className="new-heading">{t("composer.new.empty.heading")}</h3>
        <div className="cmp-sheet-grid">
          {EMPTY.map((shape) => (
            <button key={shape} type="button" className="cmp-card is-empty" onClick={() => onStart({ kind: "empty", shape })}>
              {/* Drawn as the canvas will show it: the folder, or the square, with nothing on it yet. */}
              <span className="cmp-card-art" aria-hidden="true">
                <DesignThumb doc={empty[shape]} template={template} assets={assets} size={104} version={version} />
              </span>
              <span className="cmp-card-label">{t(`composer.new.empty.${shape}`)}</span>
              <span className="cmp-card-note">{t(`composer.new.empty.${shape}Note`)}</span>
            </button>
          ))}
          {plain.map(({ tpl, doc, template: folder }) => (
            <button key={tpl.id} type="button" className="cmp-card" onClick={() => onStart({ kind: "template", template: tpl })}>
              <span className="cmp-card-art" aria-hidden="true">
                <DesignThumb doc={doc} template={folder} assets={assets} size={104} version={version} />
              </span>
              <span className="cmp-card-label">{t(`composer.templates.${tpl.id}` as MessageKey)}</span>
              <span className="cmp-card-note">{t(`composer.templateNotes.${tpl.id}` as MessageKey)}</span>
            </button>
          ))}
        </div>
      </section>
      <section className="new-section" aria-label={t("composer.new.templatesLabel")}>
        <h3 className="new-heading">{t("composer.new.templatesHeading")}</h3>
        <div className="cmp-sheet-grid">
          {designed.map(({ tpl, doc, template: folder }, i) => (
            <button key={tpl.id} type="button" className="cmp-card" style={{ animationDelay: `${Math.min(i, 14) * 18}ms` }} onClick={() => onStart({ kind: "template", template: tpl })}>
              <span className="cmp-card-art">
                <DesignThumb doc={doc} template={folder} assets={assets} size={104} version={version} />
                {tpl.photo && (
                  <span className="cmp-card-badge" aria-hidden="true" data-tip={t("composer.new.asksForPicture")}>
                    <ImageIcon size={13} />
                  </span>
                )}
              </span>
              <span className="cmp-card-label">{t(`composer.templates.${tpl.id}` as MessageKey)}</span>
            </button>
          ))}
        </div>
      </section>
    </Modal>
  );
}
