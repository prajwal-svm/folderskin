import "../../i18n/composer";
import { useT, type MessageKey } from "../../i18n";
import { Rich } from "../../i18n/Rich";
import { useLayoutEffect, useMemo, useRef } from "react";
import { ctx2d, makeCanvas, type Assets } from "../../composer/assets";
import { drawFolderView, drawFreeView, type TemplateImages } from "../../composer/composite";
import { emptyDoc, fallbackParts, type Doc, type FolderStyle, type Parts } from "../../composer/doc";
import { DRIVE_KINDS, DRIVE_STYLES, driveLabel, driveParts } from "../../composer/drives";
import { renderDoc } from "../../composer/render";
import { TEMPLATES, type Template } from "../../composer/templates";
import type { ShapeInfo } from "../../lib/shapes";
import { Modal } from "../Modal";
import { ImageIcon } from "../icons/image";
import { LoaderIcon } from "../icons/loader";

/** A design drawn small: on the folder or drive, or as it is for a free icon. */
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
    // As the canvas shows it: what's see-through shows the folder's or the drive's shape, or the icon's square.
    if (template && doc.shape !== "free") drawFolderView(ctx, design, template, px, makeCanvas(px, px));
    else drawFreeView(ctx, design, px);
  }, [doc, template, assets, size, version]);
  return <canvas ref={ref} className="cmp-design-thumb" style={{ width: size, height: size }} aria-hidden="true" />;
}

/** How a new design starts: empty (as a folder, a free icon or on a drive), or from a template. */
export type Start = { kind: "empty"; shape: "folder" | "free" } | { kind: "drive"; drive: string } | { kind: "template"; template: Template };

/** The two empty starts; each is named by `composer.new.empty.<shape>` with a note under it. */
const EMPTY: ("folder" | "free")[] = ["folder", "free"];

/** Where a surface's layers are kept: a folder's under its style, a drive's under `drive:<id>`. */
export const surfaceKey = (doc: Pick<Doc, "shape" | "style" | "drive">) => (doc.drive && doc.shape !== "folder" ? `drive:${doc.drive}` : doc.style);

/** Every folder's and drive's layers and parts, once loaded (null when they couldn't be). */
export type Surfaces = Partial<Record<string, { images: TemplateImages; parts: Parts } | null>>;

/**
 * A drive to start a design on, by what it is ("External drive"), or for the drive that's chosen
 * by its own `name` with what it is under it.
 */
function DriveCard({ id, picture, name, onStart }: { id: string; picture: string | undefined; name?: string; onStart: (start: Start) => void }) {
  return (
    <button type="button" className="cmp-card is-drive" data-drive={id} onClick={() => onStart({ kind: "drive", drive: id })}>
      <span className="cmp-card-art" aria-hidden="true">
        {picture ? (
          <img src={picture} alt="" draggable={false} />
        ) : (
          <span className="cmp-card-wait">
            <LoaderIcon size={16} />
          </span>
        )}
      </span>
      {name ? (
        <>
          <span className="cmp-card-label cmp-card-name" data-tip={name} data-tip-overflow>
            {name}
          </span>
          <span className="cmp-card-note">{driveLabel(id)}</span>
        </>
      ) : (
        <span className="cmp-card-label">{driveLabel(id)}</span>
      )}
    </button>
  );
}

/**
 * Where every new design starts: empty, on a drive, or from one of the templates. A real dialog
 * over the whole window, so the design in progress can't be edited, or mistaken for the new one,
 * while it's open. When that design has changes that aren't saved, the dialog says so first and
 * offers to save it; choosing a start then replaces it, with no second "are you sure?".
 */
export function NewDesign({
  surfaces,
  style,
  drive,
  picked,
  shapes,
  assets,
  version,
  dirty,
  saving,
  onStart,
  onSaveFirst,
  onClose,
}: {
  surfaces: Surfaces;
  /** The folder the design in progress is on, which new designs start on. */
  style: FolderStyle;
  /** The drive a design on a drive starts on. */
  drive: string;
  /** The drive chosen on the stage, by its shape and its name, when a drive is. */
  picked: { drive: string; name: string } | null;
  /** Every shape bare, from Rust, for the drives to start on; null while they're on their way. */
  shapes: ShapeInfo[] | null;
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
        if (tpl.drive) {
          // On the drive a design on a drive starts on.
          const on = surfaces[`drive:${drive}`];
          const made = tpl.make(on?.parts ?? driveParts(drive));
          return { tpl, doc: { ...emptyDoc("drive", style, drive), layers: made.layers }, template: on?.images ?? null };
        }
        // A folder's own look is shown on that folder, the rest on the one the design is on.
        const on = tpl.style ?? style;
        const folder = surfaces[on];
        return { tpl, doc: { ...tpl.make(folder?.parts ?? fallbackParts(on)), style: on }, template: folder?.images ?? null };
      }),
    // Made again in a new language: the templates' words are the language's.
    [surfaces, style, drive, t],
  );
  const empty = useMemo(() => ({ folder: emptyDoc("folder", style), free: emptyDoc("free", style) }), [style]);
  // The Mac's, Windows' and Linux's own folders have nothing on them yet: they start empty, like a blank folder.
  const plain = templates.filter(({ tpl }) => tpl.plain);
  const designed = templates.filter(({ tpl }) => !tpl.plain);
  const template = surfaces[style]?.images ?? null;
  const pictures = useMemo(() => new Map((shapes ?? []).map((s) => [s.id, s.thumbnail ?? undefined])), [shapes]);

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
      <section className="new-section" aria-label={t("composer.new.drives.label")}>
        <h3 className="new-heading">{t("composer.new.drives.heading")}</h3>
        <p className="new-sub">{t("composer.new.drives.sub")}</p>
        {picked && (
          // The drive on the stage first, by its own name, so a design for it is one click away.
          <div className="new-drives" role="group" aria-label={t("composer.new.drives.picked")}>
            <h4 className="new-drives-heading">{t("composer.new.drives.picked")}</h4>
            <div className="cmp-drive-grid">
              <DriveCard id={picked.drive} picture={pictures.get(picked.drive)} name={picked.name} onStart={onStart} />
            </div>
          </div>
        )}
        {DRIVE_STYLES.map((system) => (
          <div key={system} className="new-drives" role="group" aria-label={t(`common.systems.${system}`)}>
            <h4 className="new-drives-heading">{t(`common.systems.${system}`)}</h4>
            <div className="cmp-drive-grid">
              {DRIVE_KINDS[system].map((kind) => {
                const id = `${system}-${kind}`;
                return <DriveCard key={id} id={id} picture={pictures.get(id)} onStart={onStart} />;
              })}
            </div>
          </div>
        ))}
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
