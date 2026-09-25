import "../../i18n/composer";
import { useT, type MessageKey } from "../../i18n";
import type { CSSProperties } from "react";
import { mix } from "../../composer/color";
import { mainColor, type Paint, type Stop } from "../../composer/doc";
import { GRADIENTS, paintCss } from "../../composer/paints";
import { ColorField } from "./ColorPicker";
import { Segmented, Slider } from "./controls";
import { PlusIcon, XIcon } from "../icons/composer";

const stopsOf = (paint: Paint): Stop[] =>
  paint.type === "solid"
    ? [
        { at: 0, color: paint.color },
        { at: 1, color: mix(paint.color, "#000000", 0.35) },
      ]
    : paint.stops;

/**
 * A layer's paint: one colour, or a gradient along a line or out from a point, with up to four
 * colours along it. Switching kind keeps the colours.
 */
export function PaintField({ value, onChange, used, label: given }: { value: Paint; onChange: (p: Paint) => void; used: string[]; label?: string }) {
  const t = useT();
  const label = given ?? t("composer.paint.colour");
  const stops = stopsOf(value);
  const setStops = (next: Stop[]) => {
    if (value.type === "linear") onChange({ ...value, stops: next });
    else if (value.type === "radial") onChange({ ...value, stops: next });
  };
  return (
    <div className="cmp-paint">
      <Segmented
        label={t("composer.paint.kindLabel", { label })}
        small
        value={value.type}
        onChange={(t) => {
          if (t === value.type) return;
          if (t === "solid") onChange({ type: "solid", color: mainColor(value) });
          else if (t === "linear") onChange({ type: "linear", angle: value.type === "linear" ? value.angle : 180, stops });
          else onChange({ type: "radial", cx: 0.5, cy: 0.5, stops });
        }}
        options={[
          { value: "solid", label: t("composer.paint.solid") },
          { value: "linear", label: t("composer.paint.linear") },
          { value: "radial", label: t("composer.paint.radial") },
        ]}
      />
      {value.type === "solid" ? (
        <ColorField value={value.color} onChange={(color) => onChange({ type: "solid", color })} label={label} used={used} />
      ) : (
        <>
          <div className="cmp-gradient-bar" style={{ "--g": paintCss({ ...value, type: "linear", angle: 90 } as Paint) } as CSSProperties} aria-hidden="true" />
          <div className="cmp-stops">
            {stops.map((s, i) => (
              <div className="cmp-stop" key={i}>
                <ColorField
                  compact
                  value={s.color}
                  label={t("composer.paint.stop", { label, n: i + 1 })}
                  used={used}
                  onChange={(color) => setStops(stops.map((x, j) => (j === i ? { ...x, color } : x)))}
                />
                <Slider
                  value={s.at}
                  min={0}
                  max={100}
                  scale={100}
                  unit="%"
                  ariaLabel={t("composer.paint.stopAt", { n: i + 1 })}
                  onChange={(at) => setStops(stops.map((x, j) => (j === i ? { ...x, at } : x)).sort((a, b) => a.at - b.at))}
                />
                {stops.length > 2 && (
                  <button type="button" className="cmp-stop-x" aria-label={t("composer.paint.removeStop", { n: i + 1 })} onClick={() => setStops(stops.filter((_, j) => j !== i))}>
                    <XIcon size={12} />
                  </button>
                )}
              </div>
            ))}
            {stops.length < 4 && (
              <button
                type="button"
                className="cmp-add-stop"
                onClick={() => {
                  const a = stops[stops.length - 2];
                  const b = stops[stops.length - 1];
                  const at = (a.at + b.at) / 2;
                  setStops([...stops.slice(0, -1), { at, color: mix(a.color, b.color, 0.5) }, b]);
                }}
              >
                <PlusIcon size={13} />
                {t("composer.paint.addStop")}
              </button>
            )}
          </div>
          {value.type === "linear" ? (
            <Slider label={t("composer.inspector.angle")} value={value.angle} min={0} max={360} unit="°" onChange={(angle) => onChange({ ...value, angle })} />
          ) : (
            <>
              <Slider label={t("composer.paint.centreAcross")} value={value.cx} min={0} max={100} scale={100} unit="%" onChange={(cx) => onChange({ ...value, cx })} />
              <Slider label={t("composer.paint.centreDown")} value={value.cy} min={0} max={100} scale={100} unit="%" onChange={(cy) => onChange({ ...value, cy })} />
            </>
          )}
        </>
      )}
      <div className="cmp-presets" role="list" aria-label={t("composer.paint.gradientsLabel")}>
        {GRADIENTS.map((g) => (
          <button
            key={g.id}
            type="button"
            role="listitem"
            className="cmp-preset"
            data-tip={t(`composer.gradients.${g.id}` as MessageKey)}
            aria-label={t("composer.paint.gradientLabel", { name: t(`composer.gradients.${g.id}` as MessageKey) })}
            style={{ "--g": paintCss(g.paint) } as CSSProperties}
            onClick={() => onChange(JSON.parse(JSON.stringify(g.paint)) as Paint)}
          />
        ))}
      </div>
    </div>
  );
}
