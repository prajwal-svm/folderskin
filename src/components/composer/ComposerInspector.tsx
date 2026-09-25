import { useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import {
  BLENDS,
  centreOf,
  ICON_LOOKS,
  iconName,
  layerLabel,
  imageBox,
  isPlaced,
  NO_FX,
  patternLabel,
  shapeLabel,
  type Blend,
  type IconLayer,
  type IconLook,
  type ImageFx,
  type ImageLayer,
  type Layer,
  type Parts,
  type PatternLayer,
  type PlacedLayer,
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
import { Select } from "../Select";
import "../../i18n/composer";
import { t, useLocale, type MessageKey } from "../../i18n";
import {
  AlignCenterIcon,
  AlignLeftIcon,
  AlignRightIcon,
  CaseUpperIcon,
  ChevronDownIcon,
  FlipHIcon,
  FlipVIcon,
  InfoCircleIcon,
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

/** How far inside the front panel's edges "align left" and the others leave a layer, in canvas units. */
const ALIGN_INSET = 56;

type AlignTo = "left" | "centre" | "right" | "top" | "middle" | "bottom";

/** A small bar and box, for the align buttons: the bar is the edge the layer goes to. */
function AlignGlyph({ to }: { to: AlignTo }) {
  const bar = { left: "M4 3v18", centre: "M12 3v18", right: "M20 3v18", top: "M3 4h18", middle: "M3 12h18", bottom: "M3 20h18" }[to];
  const box = { left: "M7 8h9v8H7z", centre: "M7.5 8h9v8h-9z", right: "M8 8h9v8H8z", top: "M8 7h8v9H8z", middle: "M8 7.5h8v9H8z", bottom: "M8 8h8v9H8z" }[to];
  return (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={bar} />
      <path d={box} />
    </svg>
  );
}

function Arrange({ layer, onPatch, parts, size }: { layer: PlacedLayer; onPatch: (p: Patch, key?: string) => void; parts: Parts; size: { w: number; h: number } | null }) {
  const front = centreOf(parts.front);
  const tab = centreOf(parts.tab);
  const [fx0, fy0, fx1, fy1] = parts.front;
  const w = size?.w ?? 0;
  const h = size?.h ?? 0;
  const align: { to: AlignTo; title: string; at: Patch }[] = [
    { to: "left", title: t("composer.inspector.align.left"), at: { x: fx0 + ALIGN_INSET + w / 2 } },
    { to: "centre", title: t("composer.inspector.align.centre"), at: { x: front.x } },
    { to: "right", title: t("composer.inspector.align.right"), at: { x: fx1 - ALIGN_INSET - w / 2 } },
    { to: "top", title: t("composer.inspector.align.top"), at: { y: fy0 + ALIGN_INSET + h / 2 } },
    { to: "middle", title: t("composer.inspector.align.middle"), at: { y: front.y } },
    { to: "bottom", title: t("composer.inspector.align.bottom"), at: { y: fy1 - ALIGN_INSET - h / 2 } },
  ];
  const canStretch = layer.kind === "shape" || layer.kind === "image";
  return (
    <Section title={t("composer.inspector.place")}>
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
      <Slider label={t("composer.inspector.turn")} value={layer.rotation} min={-180} max={180} unit="°" onChange={(rotation) => onPatch({ rotation }, `rot:${layer.id}`)} />
      <div className="cmp-actions-row">
        <IconButton label={t("composer.inspector.mirrorX")} active={layer.flipX} onClick={() => onPatch({ flipX: !layer.flipX })}>
          <FlipHIcon size={15} />
        </IconButton>
        <IconButton label={t("composer.inspector.mirrorY")} active={layer.flipY} onClick={() => onPatch({ flipY: !layer.flipY })}>
          <FlipVIcon size={15} />
        </IconButton>
        <span className="cmp-actions-gap" />
        <button type="button" className="cmp-chip" data-tip={t("composer.inspector.toFrontTip")} onClick={() => onPatch({ x: front.x, y: front.y })}>
          {t("composer.inspector.toFront")}
        </button>
        <button type="button" className="cmp-chip" data-tip={t("composer.inspector.toTabTip")} onClick={() => onPatch({ x: tab.x, y: tab.y })}>
          {t("composer.inspector.toTab")}
        </button>
        <button type="button" className="cmp-chip" data-tip={t("composer.inspector.centreTip")} onClick={() => onPatch({ x: 512 })}>
          {t("composer.inspector.centre")}
        </button>
      </div>
      {size && (
        <div className="cmp-align" role="group" aria-label={t("composer.inspector.alignLabel")}>
          <span className="cmp-align-label">{t("composer.inspector.alignOn")}</span>
          <div className="cmp-align-buttons">
            {align.map((a) => (
              <IconButton key={a.to} label={a.title} onClick={() => onPatch(a.at)}>
                <AlignGlyph to={a.to} />
              </IconButton>
            ))}
          </div>
        </div>
      )}
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
    <Section title={t("composer.inspector.look")}>
      <Slider label={t("composer.inspector.opacity")} value={layer.opacity} min={0} max={100} scale={100} unit="%" onChange={(opacity) => onPatch({ opacity }, `opacity:${layer.id}`)} />
      <Field label={t("composer.inspector.blend")}>
        <Select<Blend> label={t("composer.inspector.blendLabel")} value={layer.blend} onChange={(blend) => onPatch({ blend })} options={BLENDS.map((b) => ({ value: b.id, label: t(`composer.blends.${b.id}`) }))} />
      </Field>
      {placed && (
        <>
          <Toggle
            label={t("composer.inspector.shadow")}
            on={placed.shadow !== null}
            onChange={(on) => onPatch({ shadow: on ? { color: "#00000059", blur: 26, x: 0, y: 14 } : null })}
          />
          {placed.shadow && (
            <div className="cmp-sub">
              <Field label={t("composer.inspector.colour")}>
                <ColorField value={placed.shadow.color} label={t("composer.inspector.shadowColour")} used={used} onChange={(color) => onPatch({ shadow: { ...placed.shadow!, color } }, `shadow:${layer.id}`)} />
              </Field>
              <Slider label={t("composer.inspector.softness")} value={placed.shadow.blur} min={0} max={120} onChange={(blur) => onPatch({ shadow: { ...placed.shadow!, blur } }, `shadowb:${layer.id}`)} />
              <Slider label={t("composer.inspector.across")} value={placed.shadow.x} min={-80} max={80} onChange={(x) => onPatch({ shadow: { ...placed.shadow!, x } }, `shadowx:${layer.id}`)} />
              <Slider label={t("composer.inspector.down")} value={placed.shadow.y} min={-80} max={80} onChange={(y) => onPatch({ shadow: { ...placed.shadow!, y } }, `shadowy:${layer.id}`)} />
              <button type="button" className="cmp-chip" onClick={() => onPatch({ shadow: { ...placed.shadow!, x: 0, y: 0, blur: Math.max(30, placed.shadow!.blur) } })}>
                {t("composer.inspector.glow")}
              </button>
            </div>
          )}
          <Toggle
            label={t("composer.inspector.stickerEdge")}
            hint={t("composer.inspector.stickerEdgeHint")}
            on={placed.edge !== null}
            onChange={(on) => onPatch({ edge: on ? { color: "#ffffff", width: 18 } : null })}
          />
          {placed.edge && (
            <div className="cmp-sub">
              <Field label={t("composer.inspector.colour")}>
                <ColorField value={placed.edge.color} label={t("composer.inspector.edgeColour")} used={used} onChange={(color) => onPatch({ edge: { ...placed.edge!, color } }, `edge:${layer.id}`)} />
              </Field>
              <Slider label={t("composer.inspector.width")} value={placed.edge.width} min={1} max={80} onChange={(width) => onPatch({ edge: { ...placed.edge!, width } }, `edgew:${layer.id}`)} />
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
      <Section title={t("composer.inspector.textOnFolder")}>
        <textarea
          ref={textRef}
          className="cmp-textarea"
          value={layer.text}
          rows={Math.min(5, Math.max(2, layer.text.split("\n").length))}
          maxLength={400}
          placeholder={t("composer.inspector.typeSomething")}
          aria-label={t("composer.inspector.textLabel")}
          onChange={(e) => onPatch({ text: e.target.value }, `text:${layer.id}`)}
        />
        <Field label={t("composer.inspector.font")}>
          <PopButton label={t("composer.inspector.font")} width={260} panel={(close) => <FontPicker value={layer.font} onPick={(font) => (onPatch({ font }), close())} />}>
            <span style={{ fontFamily: fontStack(layer.font) }}>{fontLabel(layer.font)}</span>
            <ChevronDownIcon size={14} />
          </PopButton>
        </Field>
        <Field label={t("composer.inspector.weight")}>
          <Select<number> label={t("composer.inspector.weightLabel")} value={layer.weight} onChange={(weight) => onPatch({ weight })} options={WEIGHTS.map((w) => ({ value: w.value, label: t(`composer.weights.${w.value}` as MessageKey) }))} />
        </Field>
        <div className="cmp-actions-row">
          <Segmented
            label={t("composer.inspector.alignment")}
            small
            value={layer.align}
            onChange={(align) => onPatch({ align })}
            options={[
              { value: "left", label: <AlignLeftIcon size={14} />, title: t("composer.inspector.textAlign.left") },
              { value: "center", label: <AlignCenterIcon size={14} />, title: t("composer.inspector.textAlign.centre") },
              { value: "right", label: <AlignRightIcon size={14} />, title: t("composer.inspector.textAlign.right") },
            ]}
          />
          <span className="cmp-actions-gap" />
          <IconButton label={t("composer.inspector.italic")} active={layer.italic} onClick={() => onPatch({ italic: !layer.italic })}>
            <ItalicIcon size={14} />
          </IconButton>
          <IconButton label={t("composer.inspector.capitals")} active={layer.upper} onClick={() => onPatch({ upper: !layer.upper })}>
            <CaseUpperIcon size={15} />
          </IconButton>
        </div>
        <Slider label={t("composer.inspector.size")} value={layer.size} min={8} max={600} onChange={(size) => onPatch({ size }, `size:${layer.id}`)} />
        <Slider label={t("composer.inspector.spacing")} value={layer.spacing} min={-20} max={100} scale={100} unit="%" onChange={(spacing) => onPatch({ spacing }, `spacing:${layer.id}`)} />
        {layer.text.includes("\n") && (
          <Slider label={t("composer.inspector.lineHeight")} value={layer.lineHeight} min={60} max={250} scale={100} unit="%" onChange={(lineHeight) => onPatch({ lineHeight }, `lh:${layer.id}`)} />
        )}
        <Slider label={t("composer.inspector.curve")} value={layer.curve} min={-100} max={100} onChange={(curve) => onPatch({ curve }, `curve:${layer.id}`)} />
      </Section>
      <Section title={t("composer.inspector.colour")}>
        <PaintField value={layer.paint} onChange={(paint) => onPatch({ paint }, `paint:${layer.id}`)} used={used} label={t("composer.inspector.textColour")} />
        <Toggle label={t("composer.inspector.outline")} on={layer.stroke !== null} onChange={(on) => onPatch({ stroke: on ? { color: "#1b1f27", width: Math.max(2, Math.round(layer.size / 18)) } : null })} />
        {layer.stroke && (
          <div className="cmp-sub">
            <Field label={t("composer.inspector.colour")}>
              <ColorField value={layer.stroke.color} label={t("composer.inspector.outlineColour")} used={used} onChange={(color) => onPatch({ stroke: { ...layer.stroke!, color } }, `stroke:${layer.id}`)} />
            </Field>
            <Slider label={t("composer.inspector.width")} value={layer.stroke.width} min={1} max={60} onChange={(width) => onPatch({ stroke: { ...layer.stroke!, width } }, `strokew:${layer.id}`)} />
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
      <Section title={t("composer.inspector.shape")}>
        <Field label={t("composer.inspector.kind")}>
          <PopButton label={t("composer.inspector.shape")} width={300} panel={(close) => <ShapeGrid value={layer.shape} onPick={(shape) => (onPatch({ shape }), close())} />}>
            {shapeLabel(layer.shape)}
            <ChevronDownIcon size={14} />
          </PopButton>
        </Field>
        {rounded && <Slider label={t("composer.inspector.rounding")} value={layer.radius} min={0} max={100} scale={100} unit="%" onChange={(radius) => onPatch({ radius }, `radius:${layer.id}`)} />}
        {pointy && (
          <Slider
            label={layer.shape === "polygon" ? t("composer.inspector.sides") : t("composer.inspector.points")}
            value={layer.points}
            min={3}
            max={layer.shape === "burst" ? 40 : 16}
            onChange={(points) => onPatch({ points: Math.round(points) }, `points:${layer.id}`)}
          />
        )}
        {(layer.shape === "star" || layer.shape === "burst" || layer.shape === "ring") && (
          <Slider
            label={layer.shape === "ring" ? t("composer.inspector.hole") : t("composer.inspector.depth")}
            value={layer.inner}
            min={5}
            max={95}
            scale={100}
            unit="%"
            onChange={(inner) => onPatch({ inner }, `inner:${layer.id}`)}
          />
        )}
      </Section>
      <Section title={t("composer.inspector.colour")}>
        <PaintField value={layer.paint} onChange={(paint) => onPatch({ paint }, `paint:${layer.id}`)} used={used} label={t("composer.inspector.shapeColour")} />
        <Toggle label={t("composer.inspector.outline")} on={layer.stroke !== null} onChange={(on) => onPatch({ stroke: on ? { color: "#ffffff", width: 10 } : null })} />
        {layer.stroke && (
          <div className="cmp-sub">
            <Field label={t("composer.inspector.colour")}>
              <ColorField value={layer.stroke.color} label={t("composer.inspector.outlineColour")} used={used} onChange={(color) => onPatch({ stroke: { ...layer.stroke!, color } }, `stroke:${layer.id}`)} />
            </Field>
            <Slider label={t("composer.inspector.width")} value={layer.stroke.width} min={1} max={80} onChange={(width) => onPatch({ stroke: { ...layer.stroke!, width } }, `strokew:${layer.id}`)} />
          </div>
        )}
      </Section>
    </>
  );
}

const FX_FIELDS: { key: keyof ImageFx; min: number; max: number; unit?: string }[] = [
  { key: "brightness", min: -100, max: 100 },
  { key: "contrast", min: -100, max: 100 },
  { key: "saturation", min: -100, max: 100 },
  { key: "hue", min: -180, max: 180, unit: "°" },
  { key: "blur", min: 0, max: 40 },
  { key: "grayscale", min: 0, max: 100, unit: "%" },
  { key: "sepia", min: 0, max: 100, unit: "%" },
  { key: "invert", min: 0, max: 100, unit: "%" },
];

function ImageSection({ layer, onPatch, parts, onReplace }: { layer: ImageLayer; onPatch: (p: Patch, key?: string) => void; parts: Parts; onReplace: (anchor: HTMLElement) => void }) {
  const fit = (cover: boolean) => onPatch(imageBox(layer.iw, layer.ih, parts, cover));
  return (
    <>
      <Section title={t("composer.inspector.picture")}>
        <div className="cmp-actions-row is-wrap">
          <button type="button" className="cmp-chip" onClick={() => fit(true)} data-tip={t("composer.inspector.coverTip")}>
            {t("composer.inspector.cover")}
          </button>
          <button type="button" className="cmp-chip" onClick={() => fit(false)} data-tip={t("composer.inspector.fitTip")}>
            {t("composer.inspector.fit")}
          </button>
          <button
            type="button"
            className="cmp-chip"
            onClick={() => onPatch({ w: layer.iw * (layer.h / layer.ih), h: layer.h })}
            data-tip={t("composer.inspector.uncropTip")}
          >
            {t("composer.inspector.uncrop")}
          </button>
          <button type="button" className="cmp-chip" onClick={(e) => onReplace(e.currentTarget)} data-tip={t("composer.inspector.replaceTip")}>
            {t("composer.inspector.replace")}
          </button>
        </div>
        <Slider label={t("composer.inspector.rounding")} value={layer.radius} min={0} max={100} scale={100} unit="%" onChange={(radius) => onPatch({ radius }, `radius:${layer.id}`)} />
      </Section>
      <Section
        title={t("composer.inspector.adjust")}
        extra={
          hasFx(layer.fx) ? (
            <button type="button" className="link-btn cmp-reset" onClick={() => onPatch({ fx: { ...NO_FX } })}>
              {t("composer.inspector.reset")}
            </button>
          ) : undefined
        }
      >
        {FX_FIELDS.map((f) => (
          <Slider
            key={f.key}
            label={t(`composer.inspector.fx.${f.key}`)}
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
    <Section title={t("composer.inspector.pattern")}>
      <Field label={t("composer.inspector.kind")}>
        <PopButton label={t("composer.inspector.pattern")} width={300} panel={(close) => <PatternGrid value={layer.pattern} onPick={(pattern) => (onPatch({ pattern }), close())} />}>
          {patternLabel(layer.pattern)}
          <ChevronDownIcon size={14} />
        </PopButton>
      </Field>
      <Field label={t("composer.inspector.colour")}>
        <ColorField value={layer.color} label={t("composer.inspector.patternColour")} used={used} onChange={(color) => onPatch({ color }, `pcolor:${layer.id}`)} />
      </Field>
      {layer.pattern !== "grain" && (
        <Field label={t("composer.inspector.behindIt")}>
          <ColorField value={layer.background} label={t("composer.inspector.behindColour")} used={used} onChange={(background) => onPatch({ background }, `pbg:${layer.id}`)} />
        </Field>
      )}
      <Slider
        label={layer.pattern === "grain" ? t("composer.inspector.grainSize") : t("composer.inspector.size")}
        value={layer.scale}
        min={layer.pattern === "grain" ? 1 : 12}
        max={layer.pattern === "grain" ? 6 : 360}
        step={layer.pattern === "grain" ? 0.5 : 1}
        onChange={(scale) => onPatch({ scale }, `scale:${layer.id}`)}
      />
      {turns && <Slider label={t("composer.inspector.angle")} value={layer.angle} min={-90} max={90} unit="°" onChange={(angle) => onPatch({ angle }, `angle:${layer.id}`)} />}
      {random && (
        <button type="button" className="cmp-chip" onClick={() => onPatch({ seed: 1 + Math.floor(Math.random() * 99999) })}>
          <ShuffleIcon size={13} />
          {t("composer.inspector.shuffle")}
        </button>
      )}
    </Section>
  );
}

/**
 * The layer's own name, for the layers list only. It's a field of its own, apart from the words a
 * text layer shows on the folder, so which one is being changed is never in doubt. Left empty, the
 * layer is named after what it is (a text layer after its words).
 */

function LayerName({ layer, index, onPatch }: { layer: Layer; index: number; onPatch: (p: Patch) => void }) {
  const auto = layerLabel({ ...layer, name: undefined } as Layer, index);
  const [draft, setDraft] = useState(layer.name ?? "");
  useEffect(() => setDraft(layer.name ?? ""), [layer.id, layer.name]);
  const commit = () => {
    const value = draft.trim();
    const next = value && value !== auto ? value : "";
    if (next !== (layer.name ?? "")) onPatch({ name: next || undefined });
    setDraft(next);
  };
  const note = layer.kind === "text" ? t("composer.inspector.nameNote.text") : t("composer.inspector.nameNote.other");
  return (
    <section className="cmp-section cmp-layer-naming">
      <Field label={t("composer.inspector.layerName")}>
        <span className="cmp-input-wrap">
          <input
            className="cmp-input"
            value={draft}
            placeholder={auto}
            maxLength={40}
            spellCheck={false}
            aria-label={t("composer.inspector.layerNameLabel")}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
              if (e.key === "Enter") e.currentTarget.blur();
              if (e.key === "Escape") {
                e.stopPropagation();
                setDraft(layer.name ?? "");
              }
            }}
          />
          <span className="cmp-input-info" tabIndex={0} role="note" aria-label={note} data-tip={note}>
            <InfoCircleIcon size={14} />
          </span>
        </span>
      </Field>
    </section>
  );
}

function IconSection({ layer, onPatch, used, onReplace }: { layer: IconLayer; onPatch: (p: Patch, key?: string) => void; used: string[]; onReplace: () => void }) {
  const looks = ICON_LOOKS.filter((l) => l.id !== "original" || layer.brand);
  return (
    <Section title={t("composer.inspector.icon")}>
      <div className="cmp-icon-now">
        <span className="cmp-icon-now-name">{iconName(layer.icon)}</span>
        <button type="button" className="cmp-chip" onClick={onReplace} data-tip={t("composer.inspector.replaceIconTip")}>
          {t("composer.inspector.replace")}
        </button>
      </div>
      <Segmented<IconLook> label={t("composer.inspector.iconLook")} value={layer.look} onChange={(look) => onPatch({ look })} options={looks.map((l) => ({ value: l.id, label: t(`composer.iconLooks.${l.id}`) }))} />
      {layer.look === "emboss" && (
        <>
          <Toggle
            label={t("composer.inspector.foldersColour")}
            hint={t("composer.inspector.foldersColourHint")}
            on={layer.auto}
            onChange={(auto) => onPatch({ auto })}
          />
          {!layer.auto && (
            <Field label={t("composer.inspector.colour")}>
              <ColorField value={layer.paint.type === "solid" ? layer.paint.color : "#ffffff"} label={t("composer.inspector.iconColour")} used={used} onChange={(color) => onPatch({ paint: { type: "solid", color } }, `icolor:${layer.id}`)} />
            </Field>
          )}
          <Slider label={t("composer.inspector.depth")} value={layer.depth} min={0} max={100} onChange={(depth) => onPatch({ depth }, `depth:${layer.id}`)} />
        </>
      )}
      {layer.look === "flat" && <PaintField value={layer.paint} onChange={(paint) => onPatch({ paint }, `ipaint:${layer.id}`)} used={used} />}
      <Slider label={t("composer.inspector.size")} value={layer.size} min={24} max={1100} onChange={(size) => onPatch({ size }, `size:${layer.id}`)} />
      {layer.style === "stroke" && (
        <Slider label={t("composer.inspector.lineWeight")} value={layer.strokeWidth} min={0.5} max={layer.viewBox / 6} step={0.25} onChange={(strokeWidth) => onPatch({ strokeWidth }, `sw:${layer.id}`)} />
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
  layer,
  onPatch,
  parts,
  used,
  textRef,
  onReplaceImage,
  onReplaceIcon,
  index,
  size,
  onFolder,
}: {
  layer: Layer | null;
  onPatch: (patch: Patch, key?: string) => void;
  parts: Parts;
  used: string[];
  textRef: RefObject<HTMLTextAreaElement | null>;
  onReplaceImage: (anchor: HTMLElement) => void;
  onReplaceIcon: () => void;
  /** Where the layer is in the stack, for its automatic name. */
  index: number;
  /** The layer's box on the canvas, for aligning it. */
  size: { w: number; h: number } | null;
  /** The design is on a folder, whose front a colour can cover alone. */
  onFolder: boolean;
}) {
  // Draws again in a new language: the sections below read it as they draw.
  useLocale();
  if (!layer) {
    return <p className="cmp-inspector-empty">{t("composer.inspector.empty")}</p>;
  }
  return (
    <div className="cmp-inspector">
      <LayerName layer={layer} index={index} onPatch={onPatch} />
      {layer.kind === "fill" && (
        <Section title={t("composer.inspector.colour")}>
          <PaintField value={layer.paint} onChange={(paint) => onPatch({ paint }, `paint:${layer.id}`)} used={used} />
          {onFolder && (
            <Field label={t("composer.inspector.covers")}>
              <Segmented<"folder" | "front">
                label={t("composer.inspector.coversLabel")}
                small
                value={layer.part === "front" ? "front" : "folder"}
                onChange={(covers) => onPatch({ part: covers === "front" ? "front" : undefined })}
                options={[
                  { value: "folder", label: t("composer.inspector.coversFolder") },
                  { value: "front", label: t("composer.inspector.coversFront"), title: t("composer.inspector.coversFrontTip") },
                ]}
              />
            </Field>
          )}
        </Section>
      )}
      {layer.kind === "pattern" && <PatternSection layer={layer} onPatch={onPatch} used={used} />}
      {layer.kind === "text" && <TextSection layer={layer} onPatch={onPatch} used={used} textRef={textRef} />}
      {layer.kind === "emoji" && (
        <Section title={t("composer.inspector.emoji")}>
          <Field label={t("composer.inspector.emoji")}>
            <PopButton label={t("composer.inspector.emoji")} width={320} className="cmp-pick-btn is-emoji" panel={(close) => <EmojiPicker onPick={(char) => (onPatch({ char }), close())} />}>
              <span className="cmp-emoji-now">{layer.char}</span>
              <ChevronDownIcon size={14} />
            </PopButton>
          </Field>
          <Slider label={t("composer.inspector.size")} value={layer.size} min={24} max={1100} onChange={(size) => onPatch({ size }, `size:${layer.id}`)} />
        </Section>
      )}
      {layer.kind === "shape" && <ShapeSection layer={layer} onPatch={onPatch} used={used} />}
      {layer.kind === "image" && <ImageSection layer={layer} onPatch={onPatch} parts={parts} onReplace={onReplaceImage} />}
      {layer.kind === "icon" && <IconSection layer={layer} onPatch={onPatch} used={used} onReplace={onReplaceIcon} />}
      {isPlaced(layer) && <Arrange layer={layer} onPatch={onPatch} parts={parts} size={size} />}
      <Effects layer={layer} onPatch={onPatch} used={used} />
    </div>
  );
}
