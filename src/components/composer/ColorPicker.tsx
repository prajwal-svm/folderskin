import { useEffect, useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
import { cssColor, hsvToRgb, parseColor, rgbToHsv, SWATCHES, toHex, type HSV } from "../../composer/color";
import { Popover } from "./Popover";

const RECENT_KEY = "folderskin.composer.recent";
const MAX_RECENT = 12;

function loadRecent(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]") as unknown;
    return Array.isArray(v) ? v.filter((c): c is string => typeof c === "string" && parseColor(c) !== null).slice(0, MAX_RECENT) : [];
  } catch {
    return [];
  }
}

/** Remembers a colour the user settled on, for the picker's Recent row. */
export function rememberColor(color: string) {
  try {
    const next = [color, ...loadRecent().filter((c) => c !== color)].slice(0, MAX_RECENT);
    localStorage.setItem(RECENT_KEY, JSON.stringify(next));
  } catch {
    // Recent colours are a convenience; without storage the row stays as it was.
  }
}

/** A dragged point inside `el`, from 0 to 1 on both axes. */
function track(e: ReactPointerEvent<HTMLElement>, onMove: (x: number, y: number) => void, onEnd?: () => void) {
  const el = e.currentTarget;
  const at = (ev: { clientX: number; clientY: number }) => {
    const r = el.getBoundingClientRect();
    onMove(Math.min(1, Math.max(0, (ev.clientX - r.left) / r.width)), Math.min(1, Math.max(0, (ev.clientY - r.top) / r.height)));
  };
  el.setPointerCapture(e.pointerId);
  at(e);
  const move = (ev: PointerEvent) => at(ev);
  const up = () => {
    el.removeEventListener("pointermove", move);
    el.removeEventListener("pointerup", up);
    el.removeEventListener("pointercancel", up);
    onEnd?.();
  };
  el.addEventListener("pointermove", move);
  el.addEventListener("pointerup", up);
  el.addEventListener("pointercancel", up);
}

/**
 * Any colour, with or without transparency: a saturation and brightness square, a hue strip, an
 * opacity strip, the hex code, and swatches (a palette, the colours already in the design and
 * the ones used lately).
 */
export function ColorPicker({ value, onChange, alpha = true, used = [] }: { value: string; onChange: (hex: string) => void; alpha?: boolean; used?: string[] }) {
  const c = parseColor(value) ?? { r: 0, g: 0, b: 0, a: 1 };
  // The hue is kept here too, so it isn't lost when the colour goes grey or black.
  const [hsv, setHsv] = useState<HSV>(() => rgbToHsv(c));
  const [hex, setHex] = useState(value);
  const [recent] = useState(loadRecent);
  const last = useRef(value);

  useEffect(() => {
    if (value === last.current) return;
    last.current = value;
    const next = parseColor(value);
    if (!next) return;
    const h = rgbToHsv(next);
    setHsv((old) => ({ h: h.s === 0 || h.v === 0 ? old.h : h.h, s: h.s, v: h.v }));
    setHex(value);
  }, [value]);

  const emit = (next: HSV, a = c.a) => {
    setHsv(next);
    const out = toHex(hsvToRgb(next, alpha ? a : 1));
    last.current = out;
    setHex(out);
    onChange(out);
  };

  const pure = toHex(hsvToRgb({ h: hsv.h, s: 1, v: 1 }));
  const opaque = toHex({ ...c, a: 1 });
  const swatch = (color: string, label: string) => (
    <button
      key={`${label}${color}`}
      type="button"
      className="cmp-swatch"
      style={{ "--c": cssColor(color) } as CSSProperties}
      aria-label={`${label} ${color}`}
      data-tip={color}
      onClick={() => {
        const p = parseColor(color);
        if (!p) return;
        const h = rgbToHsv(p);
        const out = toHex(alpha ? p : { ...p, a: 1 });
        setHsv((old) => ({ h: h.s === 0 ? old.h : h.h, s: h.s, v: h.v }));
        setHex(out);
        last.current = out;
        onChange(out);
        rememberColor(out);
      }}
    />
  );

  return (
    <div className="cmp-picker">
      <div
        className="cmp-sv"
        style={{ "--hue": pure } as CSSProperties}
        onPointerDown={(e) => track(e, (x, y) => emit({ h: hsv.h, s: x, v: 1 - y }), () => rememberColor(last.current))}
        role="slider"
        aria-label="saturation and brightness"
        aria-valuenow={Math.round(hsv.s * 100)}
      >
        <span className="cmp-sv-knob" style={{ left: `${hsv.s * 100}%`, top: `${(1 - hsv.v) * 100}%`, background: opaque }} />
      </div>
      <div
        className="cmp-strip cmp-hue"
        onPointerDown={(e) => track(e, (x) => emit({ ...hsv, h: x * 360 }), () => rememberColor(last.current))}
        role="slider"
        aria-label="hue"
        aria-valuenow={Math.round(hsv.h)}
      >
        <span className="cmp-strip-knob" style={{ left: `${(hsv.h / 360) * 100}%`, background: pure }} />
      </div>
      {alpha && (
        <div
          className="cmp-strip cmp-alpha"
          style={{ "--c": opaque } as CSSProperties}
          onPointerDown={(e) => track(e, (x) => emit(hsv, Math.round(x * 100) / 100), () => rememberColor(last.current))}
          role="slider"
          aria-label="opacity"
          aria-valuenow={Math.round(c.a * 100)}
        >
          <span className="cmp-strip-knob" style={{ left: `${c.a * 100}%`, background: cssColor(value) }} />
        </div>
      )}
      <div className="cmp-picker-row">
        <span className="cmp-picker-now" style={{ "--c": cssColor(value) } as CSSProperties} aria-hidden="true" />
        <input
          className="cmp-hex"
          value={hex}
          spellCheck={false}
          aria-label="colour code"
          onChange={(e) => {
            setHex(e.target.value);
            const p = parseColor(e.target.value);
            if (!p) return;
            const out = toHex(alpha ? p : { ...p, a: 1 });
            const h = rgbToHsv(p);
            setHsv((old) => ({ h: h.s === 0 ? old.h : h.h, s: h.s, v: h.v }));
            last.current = out;
            onChange(out);
          }}
          onBlur={() => {
            setHex(value);
            rememberColor(value);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
          }}
        />
        {alpha && <span className="cmp-picker-alpha">{Math.round(c.a * 100)}%</span>}
      </div>
      <div className="cmp-swatches">
        {SWATCHES.map((s) => swatch(s, "use"))}
        {alpha && (
          <button
            type="button"
            className="cmp-swatch is-clear"
            aria-label="no colour"
            data-tip="See-through"
            onClick={() => {
              const out = toHex({ ...c, a: 0 });
              last.current = out;
              setHex(out);
              onChange(out);
            }}
          />
        )}
      </div>
      {used.length > 0 && (
        <>
          <p className="cmp-picker-title">In this design</p>
          <div className="cmp-swatches">{used.slice(0, 18).map((s) => swatch(s, "use"))}</div>
        </>
      )}
      {recent.length > 0 && (
        <>
          <p className="cmp-picker-title">Recent</p>
          <div className="cmp-swatches">{recent.map((s) => swatch(s, "use recent"))}</div>
        </>
      )}
    </div>
  );
}

/** A colour well that opens the picker. */
export function ColorField({
  value,
  onChange,
  label,
  alpha = true,
  used,
  compact,
}: {
  value: string;
  onChange: (hex: string) => void;
  label: string;
  alpha?: boolean;
  used?: string[];
  compact?: boolean;
}) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const c = parseColor(value);
  const shown = c ? (c.a >= 1 ? toHex(c).toUpperCase() : `${toHex({ ...c, a: 1 }).toUpperCase()} · ${Math.round(c.a * 100)}%`) : value;
  return (
    <>
      <button
        type="button"
        className={compact ? "cmp-well is-compact" : "cmp-well"}
        aria-label={`${label}: ${shown}`}
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        data-tip={compact ? `${label}: ${shown}` : undefined}
        onClick={(e) => setAnchor(anchor ? null : e.currentTarget)}
      >
        <span className={c && c.a === 0 ? "cmp-well-chip is-clear" : "cmp-well-chip"} style={{ "--c": cssColor(value) } as CSSProperties} />
        {!compact && <span className="cmp-well-text">{shown}</span>}
      </button>
      {anchor && (
        <Popover anchor={anchor} onClose={() => setAnchor(null)} width={252} label={label} align="end">
          <ColorPicker value={value} onChange={onChange} alpha={alpha} used={used} />
        </Popover>
      )}
    </>
  );
}
