import "../../i18n/composer";
import { t, useLocale } from "../../i18n";
import { partLabel } from "../../composer/base";
import { HOME_FRAME, isHome, MAX_FRAME_SCALE, MIN_FRAME_SCALE, type Frame, type PartEdit } from "../../composer/doc";
import { ColorField } from "./ColorPicker";
import { Field, Section, Slider, Toggle } from "./controls";

/** Changes one part: `color: undefined` gives it back its own colour. */
export type PartPatch = (patch: PartEdit, key?: string) => void;

/** Where the whole folder or drive sits, turned and sized: the same controls wherever they show. */
function FrameSection({ title, frame, onFrame }: { title: string; frame: Frame; onFrame: (frame: Frame, key?: string) => void }) {
  return (
    <Section
      title={title}
      extra={
        !isHome(frame) ? (
          <button type="button" className="link-btn cmp-reset" onClick={() => onFrame(HOME_FRAME)}>
            {t("composer.inspector.reset")}
          </button>
        ) : undefined
      }
    >
      <Slider label={t("composer.inspector.turn")} value={frame.rotation} min={-180} max={180} unit="°" onChange={(rotation) => onFrame({ ...frame, rotation }, "frame-turn")} />
      <Slider
        label={t("composer.inspector.size")}
        value={frame.scale}
        min={MIN_FRAME_SCALE * 100}
        max={MAX_FRAME_SCALE * 100}
        scale={100}
        unit="%"
        onChange={(scale) => onFrame({ ...frame, scale }, "frame-size")}
      />
      {(frame.x !== 0 || frame.y !== 0) && (
        <div className="cmp-actions-row">
          <button type="button" className="cmp-chip" data-tip={t("composer.base.backInPlaceTip")} onClick={() => onFrame({ ...frame, x: 0, y: 0 })}>
            {t("composer.base.backInPlace")}
          </button>
        </div>
      )}
    </Section>
  );
}

/**
 * The settings of the folder or drive under the design. For one of its parts: its colour (its own,
 * or one it was given), how see-through it is and whether it shows, and the whole folder's or
 * drive's place under that. For the whole of it: its turn, its size and where it sits, and the
 * way back to its parts as FolderSkin draws them.
 */
export function BaseInspector({
  name,
  drive,
  part,
  edit,
  surface,
  color,
  frame,
  used,
  removed,
  edited,
  onPart,
  onFrame,
  onRestore,
  onResetParts,
}: {
  /** What the folder or drive is: "USB stick", "Mac folder". */
  name: string;
  drive: boolean;
  /** The part being changed, or null for the whole folder or drive. */
  part: string | null;
  edit: PartEdit;
  /** The design shows on the part: its colour takes the place of the design's backgrounds there. */
  surface: boolean;
  /** The colour the part shows now, for its colour well. */
  color: string;
  frame: Frame;
  used: string[];
  /** How many parts were removed, to bring back. */
  removed: number;
  /** Whether any part was changed at all. */
  edited: boolean;
  onPart: PartPatch;
  onFrame: (frame: Frame, key?: string) => void;
  onRestore: () => void;
  onResetParts: () => void;
}) {
  useLocale();
  if (part === null) {
    return (
      <div className="cmp-inspector">
        <FrameSection title={drive ? t("composer.base.driveTitle", { name }) : t("composer.base.folderTitle", { name })} frame={frame} onFrame={onFrame} />
        {(removed > 0 || edited) && (
          <Section title={t("composer.base.partsTitle")}>
            <div className="cmp-actions-row is-wrap">
              {removed > 0 && (
                <button type="button" className="cmp-chip" onClick={onRestore}>
                  {t("composer.base.restore", { count: removed })}
                </button>
              )}
              {edited && (
                <button type="button" className="cmp-chip" data-tip={t("composer.base.resetPartsTip")} onClick={onResetParts}>
                  {t("composer.base.resetParts")}
                </button>
              )}
            </div>
          </Section>
        )}
        <p className="cmp-inspector-note">{drive ? t("composer.base.groupNoteDrive") : t("composer.base.groupNoteFolder")}</p>
      </div>
    );
  }
  const label = partLabel(part);
  const hint = surface ? (drive ? t("composer.base.faceColourHint") : t("composer.base.panelColourHint")) : undefined;
  return (
    <div className="cmp-inspector">
      <Section
        title={label}
        extra={
          edit.color ? (
            <button type="button" className="link-btn cmp-reset" onClick={() => onPart({ color: undefined })}>
              {t("composer.base.ownColour")}
            </button>
          ) : undefined
        }
      >
        <Field label={t("composer.inspector.colour")} hint={hint}>
          <ColorField value={color} alpha={false} label={t("composer.base.colourLabel", { name: label })} used={used} onChange={(c) => onPart({ color: c.slice(0, 7) }, `part-color:${part}`)} />
        </Field>
        <Slider label={t("composer.inspector.opacity")} value={edit.opacity ?? 1} min={0} max={100} scale={100} unit="%" onChange={(opacity) => onPart({ opacity }, `part-opacity:${part}`)} />
        <Toggle label={t("composer.base.shows")} on={!edit.hidden} onChange={(on) => onPart({ hidden: !on })} />
      </Section>
      <FrameSection title={drive ? t("composer.base.driveTitle", { name }) : t("composer.base.folderTitle", { name })} frame={frame} onFrame={onFrame} />
    </div>
  );
}
