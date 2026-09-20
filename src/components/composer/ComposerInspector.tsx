import { useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import {
  BLENDS,
  centreOf,
  imageBox,
  isPlaced,
  NO_FX,
  patternLabel,
  shapeLabel,
  type Blend,
  type Doc,
  type ImageFx,
  type ImageLayer,
  type Layer,
  type Parts,
  type PatternLayer,
  type PlacedLayer,
  type Shape,
  type ShapeLayer,
  type TextLayer,
} from "../../composer/doc";
import { fontLabel, fontStack, WEIGHTS } from "../../composer/fonts";
import { hasFx } from "../../composer/imagefx";
import { ColorField } from "./ColorPicker";
import { Field, IconButton, Section, Segmented, Slider, Toggle } from "./controls";
import { PaintField } from "./PaintField";
import { EmojiPicker, FontPicker, PatternGrid, ShapeGrid } from "./pickers";
import { Popover } from "./Popover";
import {
  AlignCenterIcon,
  AlignLeftIcon,
  AlignRightIcon,
  CaseUpperIcon,
  ChevronDownIcon,
  FlipHIcon,
  FlipVIcon,
  ItalicIcon,
  ShuffleIcon,
} from "../icons/composer";

export type Patch = Record<string, unknown>;

/** A button that opens a popover under itself. */
function PopButton({ label, children, panel, width = 280, className }: { label: string; children: ReactNode; panel: (close: () => void) => ReactNode; width?: number; className?: string }) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  return (
    <>
      <button type="button" className={className ?? "cmp-pick-btn"} aria-haspopup="dialog" aria-expanded={anchor !== null} onClick={(e) => setAnchor(anchor ? null : e.currentTarget)}>
        {children}
      </button>
      {anchor && (
        <Popover anchor={anchor} onClose={() => setAnchor(null)} width={width} label={label} align="end">
          {panel(() => setAnchor(null))}
        </Popover>
      )}
    </>
  );
}

function Arrange({ layer, onPatch, parts }: { layer: PlacedLayer; onPatch: (p: Patch, key?: string) => void; parts: Parts }) {
  const front = centreOf(parts.front);
  const tab = centreOf(parts.tab);
  const canStretch = layer.kind === "shape" || layer.kind === "image";
  return (
    <Section title="Place">
      <div className="cmp-pair">
        <label className="cmp-mini-field">
          <span>X</span>
          <NumberBox value={layer.x} onChange={(x) => onPatch({ x }, `x:${layer.id}`)} />
        </label>
        <label className="cmp-mini-field">
          <span>Y</span>
          <NumberBox value={layer.y} onChange={(y) => onPatch({ y }, `y:${layer.id}`)} />
        </label>
      </div>
      {canStretch && (
        <div className="cmp-pair">
          <label className="cmp-mini-field">
            <span>W</span>
            <NumberBox value={(layer as ShapeLayer).w} min={8} onChange={(w) => onPatch({ w }, `w:${layer.id}`)} />
          </label>
          <label className="cmp-mini-field">
            <span>H</span>
            <NumberBox value={(layer as ShapeLayer).h} min={8} onChange={(h) => onPatch({ h }, `h:${layer.id}`)} />
          </label>
        </div>
      )}
      <Slider label="Turn" value={layer.rotation} min={-180} max={180} unit="°" onChange={(rotation) => onPatch({ rotation }, `rot:${layer.id}`)} />
      <div className="cmp-actions-row">
        <IconButton label="Mirror left to right" active={layer.flipX} onClick={() => onPatch({ flipX: !layer.flipX })}>
          <FlipHIcon size={15} />
        </IconButton>
        <IconButton label="Mirror top to bottom" active={layer.flipY} onClick={() => onPatch({ flipY: !layer.flipY })}>
          <FlipVIcon size={15} />
        </IconButton>
        <span className="cmp-actions-gap" />
        <button type="button" className="cmp-chip" title="Put it in the middle of the folder's front" onClick={() => onPatch({ x: front.x, y: front.y })}>
          Front
        </button>
        <button type="button" className="cmp-chip" title="Put it on the folder's tab" onClick={() => onPatch({ x: tab.x, y: tab.y })}>
          Tab
        </button>
        <button type="button" className="cmp-chip" title="Centre it across" onClick={() => onPatch({ x: 512 })}>
          Centre
        </button>
      </div>
    </Section>
  );
}

/** A number field that keeps what's being typed until it's a number. */
function NumberBox({ value, onChange, min = -4096, max = 4096 }: { value: number; onChange: (v: number) => void; min?: number; max?: number }) {
  const [draft, setDraft] = useState(String(Math.round(value)));
  const typing = useRef(false);
  useEffect(() => {
    if (!typing.current) setDraft(String(Math.round(value)));
  }, [value]);
  return (
    <input
      className="cmp-number"
      inputMode="numeric"
      value={draft}
      onFocus={(e) => {
        typing.current = true;
        e.currentTarget.select();
      }}
      onBlur={() => {
        typing.current = false;
        setDraft(String(Math.round(value)));
      }}
      onChange={(e) => {
        setDraft(e.target.value);
        const n = Number(e.target.value);
        if (e.target.value.trim() !== "" && Number.isFinite(n)) onChange(Math.min(max, Math.max(min, n)));
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
        if (e.key === "ArrowUp" || e.key === "ArrowDown") {
          e.preventDefault();
          onChange(Math.min(max, Math.max(min, Math.round(value) + (e.key === "ArrowUp" ? 1 : -1) * (e.shiftKey ? 10 : 1))));
        }
      }}
    />
  );
}

function Effects({ layer, onPatch, used }: { layer: Layer; onPatch: (p: Patch, key?: string) => void; used: string[] }) {
  const placed = isPlaced(layer) ? layer : null;
  return (
    <Section title="Look">
      <Slider label="Opacity" value={layer.opacity} min={0} max={100} scale={100} unit="%" onChange={(opacity) => onPatch({ opacity }, `opacity:${layer.id}`)} />
      <Field label="Blend">
        <select className="cmp-select" value={layer.blend} aria-label="blend" onChange={(e) => onPatch({ blend: e.target.value as Blend })}>
          {BLENDS.map((b) => (
            <option key={b.id} value={b.id}>
              {b.label}
            </option>
          ))}
        </select>
      </Field>
      {placed && (
        <>
          <Toggle
            label="Shadow"
            on={placed.shadow !== null}
            onChange={(on) => onPatch({ shadow: on ? { color: "#00000059", blur: 26, x: 0, y: 14 } : null })}
          />
          {placed.shadow && (
            <div className="cmp-sub">
              <Field label="Colour">
                <ColorField value={placed.shadow.color} label="shadow colour" used={used} onChange={(color) => onPatch({ shadow: { ...placed.shadow!, color } }, `shadow:${layer.id}`)} />
              </Field>
              <Slider label="Softness" value={placed.shadow.blur} min={0} max={120} onChange={(blur) => onPatch({ shadow: { ...placed.shadow!, blur } }, `shadowb:${layer.id}`)} />
              <Slider label="Across" value={placed.shadow.x} min={-80} max={80} onChange={(x) => onPatch({ shadow: { ...placed.shadow!, x } }, `shadowx:${layer.id}`)} />
              <Slider label="Down" value={placed.shadow.y} min={-80} max={80} onChange={(y) => onPatch({ shadow: { ...placed.shadow!, y } }, `shadowy:${layer.id}`)} />
              <button type="button" className="cmp-chip" onClick={() => onPatch({ shadow: { ...placed.shadow!, x: 0, y: 0, blur: Math.max(30, placed.shadow!.blur) } })}>
                Make it a glow
              </button>
            </div>
          )}
          <Toggle
            label="Sticker edge"
            hint="A border that follows its outline, like a die-cut sticker"
            on={placed.edge !== null}
            onChange={(on) => onPatch({ edge: on ? { color: "#ffffff", width: 18 } : null })}
          />
          {placed.edge && (
            <div className="cmp-sub">
              <Field label="Colour">
                <ColorField value={placed.edge.color} label="edge colour" used={used} onChange={(color) => onPatch({ edge: { ...placed.edge!, color } }, `edge:${layer.id}`)} />
              </Field>
              <Slider label="Width" value={placed.edge.width} min={1} max={80} onChange={(width) => onPatch({ edge: { ...placed.edge!, width } }, `edgew:${layer.id}`)} />
            </div>
          )}
        </>
      )}
    </Section>
  );
}

function TextSection({ layer, onPatch, used, textRef }: { layer: TextLayer; onPatch: (p: Patch, key?: string) => void; used: string[]; textRef: RefObject<HTMLTextAreaElement | null> }) {
  return (
    <>
      <Section title="Words">
        <textarea
          ref={textRef}
          className="cmp-textarea"
          value={layer.text}
          rows={Math.min(5, Math.max(2, layer.text.split("\n").length))}
          maxLength={400}
          placeholder="Type something"
          aria-label="words"
          onChange={(e) => onPatch({ text: e.target.value }, `text:${layer.id}`)}
        />
        <Field label="Font">
          <PopButton label="Font" width={260} panel={(close) => <FontPicker value={layer.font} onPick={(font) => (onPatch({ font }), close())} />}>
            <span style={{ fontFamily: fontStack(layer.font) }}>{fontLabel(layer.font)}</span>
            <ChevronDownIcon size={14} />
          </PopButton>
        </Field>
        <Field label="Weight">
          <select className="cmp-select" value={layer.weight} aria-label="weight" onChange={(e) => onPatch({ weight: Number(e.target.value) })}>
            {WEIGHTS.map((w) => (
              <option key={w.value} value={w.value}>
                {w.label}
              </option>
            ))}
          </select>
        </Field>
        <div className="cmp-actions-row">
          <Segmented
            label="alignment"
            small
            value={layer.align}
            onChange={(align) => onPatch({ align })}
            options={[
              { value: "left", label: <AlignLeftIcon size={14} />, title: "Left" },
              { value: "center", label: <AlignCenterIcon size={14} />, title: "Centre" },
              { value: "right", label: <AlignRightIcon size={14} />, title: "Right" },
            ]}
          />
          <span className="cmp-actions-gap" />
          <IconButton label="Italic" active={layer.italic} onClick={() => onPatch({ italic: !layer.italic })}>
            <ItalicIcon size={14} />
          </IconButton>
          <IconButton label="Capitals" active={layer.upper} onClick={() => onPatch({ upper: !layer.upper })}>
            <CaseUpperIcon size={15} />
          </IconButton>
        </div>
        <Slider label="Size" value={layer.size} min={8} max={600} onChange={(size) => onPatch({ size }, `size:${layer.id}`)} />
        <Slider label="Spacing" value={layer.spacing} min={-20} max={100} scale={100} unit="%" onChange={(spacing) => onPatch({ spacing }, `spacing:${layer.id}`)} />
        {layer.text.includes("\n") && (
          <Slider label="Line height" value={layer.lineHeight} min={60} max={250} scale={100} unit="%" onChange={(lineHeight) => onPatch({ lineHeight }, `lh:${layer.id}`)} />
        )}
        <Slider label="Curve" value={layer.curve} min={-100} max={100} onChange={(curve) => onPatch({ curve }, `curve:${layer.id}`)} />
      </Section>
      <Section title="Colour">
        <PaintField value={layer.paint} onChange={(paint) => onPatch({ paint }, `paint:${layer.id}`)} used={used} label="Text colour" />
        <Toggle label="Outline" on={layer.stroke !== null} onChange={(on) => onPatch({ stroke: on ? { color: "#1b1f27", width: Math.max(2, Math.round(layer.size / 18)) } : null })} />
        {layer.stroke && (
          <div className="cmp-sub">
            <Field label="Colour">
              <ColorField value={layer.stroke.color} label="outline colour" used={used} onChange={(color) => onPatch({ stroke: { ...layer.stroke!, color } }, `stroke:${layer.id}`)} />
            </Field>
            <Slider label="Width" value={layer.stroke.width} min={1} max={60} onChange={(width) => onPatch({ stroke: { ...layer.stroke!, width } }, `strokew:${layer.id}`)} />
          </div>
        )}
      </Section>
    </>
  );
}

function ShapeSection({ layer, onPatch, used }: { layer: ShapeLayer; onPatch: (p: Patch, key?: string) => void; used: string[] }) {
  const rounded = layer.shape === "rect" || layer.shape === "bar" || layer.shape === "bubble";
  const pointy = layer.shape === "star" || layer.shape === "burst" || layer.shape === "polygon";
  return (
    <>
      <Section title="Shape">
        <Field label="Kind">
          <PopButton label="Shape" width={300} panel={(close) => <ShapeGrid value={layer.shape} onPick={(shape) => (onPatch({ shape }), close())} />}>
            {shapeLabel(layer.shape)}
            <ChevronDownIcon size={14} />
          </PopButton>
        </Field>
        {rounded && <Slider label="Rounding" value={layer.radius} min={0} max={100} scale={100} unit="%" onChange={(radius) => onPatch({ radius }, `radius:${layer.id}`)} />}
        {pointy && (
          <Slider
            label={layer.shape === "polygon" ? "Sides" : "Points"}
            value={layer.points}
            min={3}
            max={layer.shape === "burst" ? 40 : 16}
            onChange={(points) => onPatch({ points: Math.round(points) }, `points:${layer.id}`)}
          />
        )}
        {(layer.shape === "star" || layer.shape === "burst" || layer.shape === "ring") && (
          <Slider
            label={layer.shape === "ring" ? "Hole" : "Depth"}
            value={layer.inner}
            min={5}
            max={95}
            scale={100}
            unit="%"
            onChange={(inner) => onPatch({ inner }, `inner:${layer.id}`)}
          />
        )}
      </Section>
      <Section title="Colour">
        <PaintField value={layer.paint} onChange={(paint) => onPatch({ paint }, `paint:${layer.id}`)} used={used} label="Shape colour" />
        <Toggle label="Outline" on={layer.stroke !== null} onChange={(on) => onPatch({ stroke: on ? { color: "#ffffff", width: 10 } : null })} />
        {layer.stroke && (
          <div className="cmp-sub">
            <Field label="Colour">
              <ColorField value={layer.stroke.color} label="outline colour" used={used} onChange={(color) => onPatch({ stroke: { ...layer.stroke!, color } }, `stroke:${layer.id}`)} />
            </Field>
            <Slider label="Width" value={layer.stroke.width} min={1} max={80} onChange={(width) => onPatch({ stroke: { ...layer.stroke!, width } }, `strokew:${layer.id}`)} />
          </div>
        )}
      </Section>
    </>
  );
}

const FX_FIELDS: { key: keyof ImageFx; label: string; min: number; max: number; unit?: string }[] = [
  { key: "brightness", label: "Brightness", min: -100, max: 100 },
  { key: "contrast", label: "Contrast", min: -100, max: 100 },
  { key: "saturation", label: "Colour", min: -100, max: 100 },
  { key: "hue", label: "Hue", min: -180, max: 180, unit: "°" },
  { key: "blur", label: "Blur", min: 0, max: 40 },
  { key: "grayscale", label: "Black & white", min: 0, max: 100, unit: "%" },
  { key: "sepia", label: "Sepia", min: 0, max: 100, unit: "%" },
  { key: "invert", label: "Invert", min: 0, max: 100, unit: "%" },
];

function ImageSection({ layer, onPatch, parts, onReplace }: { layer: ImageLayer; onPatch: (p: Patch, key?: string) => void; parts: Parts; onReplace: (anchor: HTMLElement) => void }) {
  const fit = (cover: boolean) => onPatch(imageBox(layer.iw, layer.ih, parts, cover));
  return (
    <>
      <Section title="Picture">
        <div className="cmp-actions-row is-wrap">
          <button type="button" className="cmp-chip" onClick={() => fit(true)} title="Cover the whole folder with it">
            Cover folder
          </button>
          <button type="button" className="cmp-chip" onClick={() => fit(false)} title="Fit it on the front, whole">
            Fit on front
          </button>
          <button
            type="button"
            className="cmp-chip"
            onClick={() => onPatch({ w: layer.iw * (layer.h / layer.ih), h: layer.h })}
            title="Undo any cropping from the sides"
          >
            Uncrop
          </button>
          <button type="button" className="cmp-chip" onClick={(e) => onReplace(e.currentTarget)}>
            Replace
          </button>
        </div>
        <Slider label="Rounding" value={layer.radius} min={0} max={100} scale={100} unit="%" onChange={(radius) => onPatch({ radius }, `radius:${layer.id}`)} />
      </Section>
      <Section
        title="Adjust"
        extra={
          hasFx(layer.fx) ? (
            <button type="button" className="link-btn cmp-reset" onClick={() => onPatch({ fx: { ...NO_FX } })}>
              Reset
            </button>
          ) : undefined
        }
      >
        {FX_FIELDS.map((f) => (
          <Slider
            key={f.key}
            label={f.label}
            value={layer.fx[f.key]}
            min={f.min}
            max={f.max}
            unit={f.unit}
            onChange={(v) => onPatch({ fx: { ...layer.fx, [f.key]: v } }, `fx-${f.key}:${layer.id}`)}
          />
        ))}
      </Section>
    </>
  );
}

function PatternSection({ layer, onPatch, used }: { layer: PatternLayer; onPatch: (p: Patch, key?: string) => void; used: string[] }) {
  const turns = layer.pattern !== "grain" && layer.pattern !== "confetti" && layer.pattern !== "dots";
  const random = layer.pattern === "grain" || layer.pattern === "confetti";
  return (
    <Section title="Pattern">
      <Field label="Kind">
        <PopButton label="Pattern" width={300} panel={(close) => <PatternGrid value={layer.pattern} onPick={(pattern) => (onPatch({ pattern }), close())} />}>
          {patternLabel(layer.pattern)}
          <ChevronDownIcon size={14} />
        </PopButton>
      </Field>
      <Field label="Colour">
        <ColorField value={layer.color} label="pattern colour" used={used} onChange={(color) => onPatch({ color }, `pcolor:${layer.id}`)} />
      </Field>
      {layer.pattern !== "grain" && (
        <Field label="Behind it">
          <ColorField value={layer.background} label="colour behind the pattern" used={used} onChange={(background) => onPatch({ background }, `pbg:${layer.id}`)} />
        </Field>
      )}
      <Slider
        label={layer.pattern === "grain" ? "Grain size" : "Size"}
        value={layer.scale}
        min={layer.pattern === "grain" ? 1 : 12}
        max={layer.pattern === "grain" ? 6 : 360}
        step={layer.pattern === "grain" ? 0.5 : 1}
        onChange={(scale) => onPatch({ scale }, `scale:${layer.id}`)}
      />
      {turns && <Slider label="Angle" value={layer.angle} min={-90} max={90} unit="°" onChange={(angle) => onPatch({ angle }, `angle:${layer.id}`)} />}
      {random && (
        <button type="button" className="cmp-chip" onClick={() => onPatch({ seed: 1 + Math.floor(Math.random() * 99999) })}>
          <ShuffleIcon size={13} />
          Shuffle
        </button>
      )}
    </Section>
  );
}

/**
 * Everything about the selected layer, in the order people reach for it: what it is (words,
 * shape, picture, pattern), its colour, where it sits, then how it looks (opacity, blend,
 * shadow, sticker edge). With nothing selected, the design as a whole. The panel around it
 * carries the layer's name and its buttons (restack, duplicate, delete).
 */
export function ComposerInspector({
  doc,
  layer,
  onPatch,
  onShape,
  parts,
  used,
  textRef,
  onReplaceImage,
}: {
  doc: Doc;
  layer: Layer | null;
  onPatch: (patch: Patch, key?: string) => void;
  onShape: (shape: Shape) => void;
  parts: Parts;
  used: string[];
  textRef: RefObject<HTMLTextAreaElement | null>;
  onReplaceImage: (anchor: HTMLElement) => void;
}) {
  if (!layer) {
    return (
      <div className="cmp-inspector">
        <Section title="Shape">
          <Segmented
            label="what it becomes"
            value={doc.shape}
            onChange={onShape}
            options={[
              { value: "folder", label: "On the folder" },
              { value: "free", label: "Free icon" },
            ]}
          />
          <p className="cmp-note">
            {doc.shape === "folder"
              ? "Your design is cut to FolderSkin's folder, with its tab, paper and edges. See-through parts stay see-through."
              : "Your design is the whole icon, any shape you like: a sticker, a badge, a big emoji."}
          </p>
        </Section>
        <Section title="Tips">
          <ul className="cmp-tips">
            <li>Click the folder to change its colour.</li>
            <li>Double-click words to edit them, or an emoji to swap it.</li>
            <li>Drag a corner to resize, the round knob to turn. Hold ⇧ for even steps.</li>
            <li>Hold ⌘ while dragging to stop things snapping into place.</li>
            <li>Drop or paste a picture straight onto the folder.</li>
          </ul>
        </Section>
      </div>
    );
  }
  return (
    <div className="cmp-inspector">
      {layer.kind === "fill" && (
        <Section title="Colour">
          <PaintField value={layer.paint} onChange={(paint) => onPatch({ paint }, `paint:${layer.id}`)} used={used} />
        </Section>
      )}
      {layer.kind === "pattern" && <PatternSection layer={layer} onPatch={onPatch} used={used} />}
      {layer.kind === "text" && <TextSection layer={layer} onPatch={onPatch} used={used} textRef={textRef} />}
      {layer.kind === "emoji" && (
        <Section title="Emoji">
          <Field label="Emoji">
            <PopButton label="Emoji" width={320} className="cmp-pick-btn is-emoji" panel={(close) => <EmojiPicker onPick={(char) => (onPatch({ char }), close())} />}>
              <span className="cmp-emoji-now">{layer.char}</span>
              <ChevronDownIcon size={14} />
            </PopButton>
          </Field>
          <Slider label="Size" value={layer.size} min={24} max={1100} onChange={(size) => onPatch({ size }, `size:${layer.id}`)} />
        </Section>
      )}
      {layer.kind === "shape" && <ShapeSection layer={layer} onPatch={onPatch} used={used} />}
      {layer.kind === "image" && <ImageSection layer={layer} onPatch={onPatch} parts={parts} onReplace={onReplaceImage} />}
      {isPlaced(layer) && <Arrange layer={layer} onPatch={onPatch} parts={parts} />}
      <Effects layer={layer} onPatch={onPatch} used={used} />
    </div>
  );
}
