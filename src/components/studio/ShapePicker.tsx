import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { Shape } from "../../state/chats";
import { groupShapes, shapeName, shapeNote, type ShapeInfo } from "../../lib/shapes";
import { Popover } from "../composer/Popover";
import { Segmented } from "../composer/controls";
import { ChevronDownIcon, ShapesIcon, TickIcon } from "../icons/composer";
import { ImageIcon } from "../icons/image";
import { useT } from "../../i18n";

/**
 * A shape's picture: the bare folder as its system draws it, or, for a free icon, which has no base
 * to show, a sign of one. With `art`, a small picture on its corner says only the art is painted,
 * for the compositor to wrap onto it.
 */
export function ShapeThumb({ shape, art = false }: { shape: ShapeInfo | undefined; art?: boolean }) {
  return (
    <span className={shape?.thumbnail ? "shape-thumb" : "shape-thumb is-free"} aria-hidden="true">
      {shape?.thumbnail ? <img src={shape.thumbnail} alt="" draggable={false} /> : <ShapesIcon size={15} />}
      {art && (
        <span className="shape-thumb-art">
          <ImageIcon size={9} />
        </span>
      )}
    </span>
  );
}

/**
 * What the chat's pictures are for, in the prompt's bar: the shape's picture and name, which open
 * the shapes to pick another, and for a shape with a base, whether the model paints the whole of it
 * or just the art. "@" in the box picks a shape too (PromptBox.tsx).
 */
export function ShapePicker({
  shapes,
  shape,
  make,
  onShape,
  onMake,
  flash,
}: {
  shapes: ShapeInfo[];
  shape: ShapeInfo | undefined;
  make: Shape;
  onShape: (id: string) => void;
  onMake: (make: Shape) => void;
  /** Changes when a shape is picked with "@", so the chip shows it changed. */
  flash: number;
}) {
  const t = useT();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const chip = useRef<HTMLButtonElement>(null);
  // Picked with "@", the chip lights up for a moment, as the box does when a brief goes in.
  useEffect(() => {
    if (!flash) return;
    const root = getComputedStyle(document.documentElement);
    chip.current?.animate(
      [
        { backgroundColor: root.getPropertyValue("--accent-soft").trim() },
        { backgroundColor: root.getPropertyValue("--well").trim() },
      ],
      { duration: 800, easing: "cubic-bezier(0.2, 0.8, 0.2, 1)" },
    );
  }, [flash]);
  const name = shape ? shapeName(shape) : "";
  const art = Boolean(shape?.whole) && make === "skin";
  const close = () => setAnchor(null);
  const wholeLabel = shape?.family === "drive" ? t("ai.prompt.shape.drive") : t("ai.prompt.shape.folder");
  const wholeTip = shape?.family === "drive" ? t("ai.prompt.shape.driveTip") : t("ai.prompt.shape.folderTip");

  // The arrow keys go from shape to shape, as in any group of choices.
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const options = [...(list.current?.querySelectorAll<HTMLButtonElement>("[role='radio']") ?? [])];
    const at = options.indexOf(document.activeElement as HTMLButtonElement);
    const next = options[(at + (e.key === "ArrowDown" ? 1 : -1) + options.length) % options.length];
    next?.focus();
    e.preventDefault();
  };

  return (
    <>
      <button
        type="button"
        ref={chip}
        className="shape-chip"
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        aria-label={t("ai.shape.label", { shape: name })}
        data-tip={art ? t("ai.shape.tipArt") : t("ai.shape.tip")}
        disabled={!shape}
        onMouseDown={(e) => e.preventDefault()}
        onClick={(e) => setAnchor(anchor ? null : e.currentTarget)}
      >
        <ShapeThumb shape={shape} art={art} />
        <span className="shape-chip-name">{name}</span>
        <ChevronDownIcon size={13} />
      </button>
      {anchor && (
        <Popover anchor={anchor} onClose={close} width={296} label={t("ai.shape.menuLabel")} className="shape-pop">
          <div className="shape-list" role="radiogroup" aria-label={t("ai.shape.menuLabel")} ref={list} onKeyDown={onKey}>
            {groupShapes(shapes).map((group) => (
              <div className="shape-group" key={group.family}>
                {group.shapes.map((s) => {
                  const on = s.id === shape?.id;
                  return (
                    <button
                      key={s.id}
                      type="button"
                      role="radio"
                      aria-checked={on}
                      className={on ? "shape-option is-on" : "shape-option"}
                      onClick={() => {
                        close();
                        onShape(s.id);
                      }}
                    >
                      <ShapeThumb shape={s} />
                      <span className="shape-option-text">
                        <span className="shape-option-name">{shapeName(s)}</span>
                        {shapeNote(s) && <span className="shape-option-note">{shapeNote(s)}</span>}
                      </span>
                      {on && <TickIcon size={15} />}
                    </button>
                  );
                })}
              </div>
            ))}
          </div>
          {shape?.whole && (
            <div className="shape-make">
              <Segmented<Shape>
                label={t("ai.prompt.shapeLabel")}
                small
                value={make}
                onChange={onMake}
                options={[
                  { value: "folder", label: wholeLabel, title: wholeTip },
                  { value: "skin", label: t("ai.prompt.shape.skin"), title: t("ai.prompt.shape.skinTip") },
                ]}
              />
            </div>
          )}
        </Popover>
      )}
    </>
  );
}
