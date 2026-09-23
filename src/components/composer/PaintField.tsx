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
export function PaintField({ value, onChange, used, label = "Colour" }: { value: Paint; onChange: (p: Paint) => void; used: string[]; label?: string }) {
  const stops = stopsOf(value);
  const setStops = (next: Stop[]) => {
    if (value.type === "linear") onChange({ ...value, stops: next });
    else if (value.type === "radial") onChange({ ...value, stops: next });
  };
  return (
    <div className="cmp-paint">
      <Segmented
        label={`${label} kind`}
        small
        value={value.type}
        onChange={(t) => {
          if (t === value.type) return;
          if (t === "solid") onChange({ type: "solid", color: mainColor(value) });
          else if (t === "linear") onChange({ type: "linear", angle: value.type === "linear" ? value.angle : 180, stops });
          else onChange({ type: "radial", cx: 0.5, cy: 0.5, stops });
        }}
        options={[
          { value: "solid", label: "Solid" },
          { value: "linear", label: "Gradient" },
          { value: "radial", label: "Radial" },
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
                  label={`${label} ${i + 1}`}
                  used={used}
                  onChange={(color) => setStops(stops.map((x, j) => (j === i ? { ...x, color } : x)))}
                />
                <Slider
                  value={s.at}
                  min={0}
                  max={100}
                  scale={100}
                  unit="%"
                  ariaLabel={`where colour ${i + 1} sits`}
                  onChange={(at) => setStops(stops.map((x, j) => (j === i ? { ...x, at } : x)).sort((a, b) => a.at - b.at))}
                />
                {stops.length > 2 && (
                  <button type="button" className="cmp-stop-x" aria-label={`remove colour ${i + 1}`} onClick={() => setStops(stops.filter((_, j) => j !== i))}>
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
                Add a colour
              </button>
            )}
          </div>
          {value.type === "linear" ? (
            <Slider label="Angle" value={value.angle} min={0} max={360} unit="°" onChange={(angle) => onChange({ ...value, angle })} />
          ) : (
            <>
              <Slider label="Centre across" value={value.cx} min={0} max={100} scale={100} unit="%" onChange={(cx) => onChange({ ...value, cx })} />
              <Slider label="Centre down" value={value.cy} min={0} max={100} scale={100} unit="%" onChange={(cy) => onChange({ ...value, cy })} />
            </>
          )}
        </>
      )}
      <div className="cmp-presets" role="list" aria-label="ready-made gradients">
        {GRADIENTS.map((g) => (
          <button
            key={g.label}
            type="button"
            role="listitem"
            className="cmp-preset"
            data-tip={g.label}
            aria-label={`${g.label} gradient`}
            style={{ "--g": paintCss(g.paint) } as CSSProperties}
            onClick={() => onChange(JSON.parse(JSON.stringify(g.paint)) as Paint)}
          />
        ))}
      </div>
    </div>
  );
}
