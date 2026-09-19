import { useLayoutEffect, useMemo, useRef } from "react";
import { ctx2d, makeCanvas, type Assets } from "../../composer/assets";
import { drawOnFolder, type TemplateImages } from "../../composer/composite";
import type { Doc, Parts } from "../../composer/doc";
import { renderDoc } from "../../composer/render";
import { TEMPLATES, type Template } from "../../composer/templates";
import { ImageIcon } from "../icons/image";
import { XIcon } from "../icons/composer";

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
    if (template && doc.shape === "folder") drawOnFolder(ctx, design, template, px, makeCanvas(px, px));
    else ctx.drawImage(design, 0, 0);
  }, [doc, template, assets, size, version]);
  return <canvas ref={ref} className="cmp-design-thumb" style={{ width: size, height: size }} aria-hidden="true" />;
}

/**
 * Where a new design starts: a plain folder, a colour, words, an emoji, glass, patterns, a photo
 * with a caption, or a free-shaped sticker. Every one is an ordinary design, changeable in
 * every way.
 */
export function TemplateSheet({
  parts,
  template,
  assets,
  onPick,
  onClose,
}: {
  parts: Parts;
  template: TemplateImages | null;
  assets: Assets;
  onPick: (t: Template) => void;
  /** Present when there's a design to go back to. */
  onClose?: () => void;
}) {
  const docs = useMemo(() => TEMPLATES.map((t) => ({ t, doc: t.make(parts) })), [parts]);
  return (
    <div className="cmp-sheet" role="dialog" aria-label="start a design">
      <div className="cmp-sheet-head">
        <div>
          <h2 className="cmp-sheet-title">Design a skin</h2>
          <p className="cmp-sheet-sub">Pick a starting point. Everything on it can be changed: colours, words, pictures, even the folder itself.</p>
        </div>
        {onClose && (
          <button type="button" className="icon-btn" aria-label="back to your design" title="Back to your design" onClick={onClose}>
            <XIcon size={15} />
          </button>
        )}
      </div>
      <div className="cmp-sheet-grid">
        {docs.map(({ t, doc }, i) => (
          <button key={t.id} type="button" className="cmp-card" style={{ animationDelay: `${Math.min(i, 14) * 22}ms` }} onClick={() => onPick(t)}>
            <span className="cmp-card-art">
              <DesignThumb doc={doc} template={template} assets={assets} size={112} />
              {t.photo && (
                <span className="cmp-card-badge" aria-hidden="true">
                  <ImageIcon size={13} />
                </span>
              )}
            </span>
            <span className="cmp-card-label">{t.label}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
