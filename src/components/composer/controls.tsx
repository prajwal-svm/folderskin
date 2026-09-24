import { useEffect, useId, useState, type CSSProperties, type ReactNode } from "react";

/** A labelled row in the inspector. */
export function Field({ label, children, hint, wide }: { label: string; children: ReactNode; hint?: string; wide?: boolean }) {
  return (
    <div className={wide ? "cmp-field is-wide" : "cmp-field"}>
      <span className="cmp-field-label">{label}</span>
      <div className="cmp-field-control">{children}</div>
      {hint && <span className="cmp-field-hint">{hint}</span>}
    </div>
  );
}

/** A group of fields with a small heading, and an optional control beside the heading. */
export function Section({ title, children, extra }: { title: string; children: ReactNode; extra?: ReactNode }) {
  return (
    <section className="cmp-section">
      <header className="cmp-section-head">
        <h3 className="cmp-section-title">{title}</h3>
        {extra}
      </header>
      <div className="cmp-section-body">{children}</div>
    </section>
  );
}

const round = (v: number, step: number) => {
  const digits = step < 1 ? Math.min(3, String(step).split(".")[1]?.length ?? 0) : 0;
  return Number(v.toFixed(digits));
};

/**
 * A slider with its value beside it, which can be typed into. `unit` follows the value (%, °);
 * `scale` turns the stored value into the shown one and back.
 */
export function Slider({
  label,
  value,
  min,
  max,
  step = 1,
  onChange,
  unit = "",
  scale = 1,
  ariaLabel,
}: {
  label?: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (v: number) => void;
  unit?: string;
  /** Shown value = stored value × scale (opacity 0–1 shows as 0–100). */
  scale?: number;
  ariaLabel?: string;
}) {
  const shown = round(value * scale, step);
  const [draft, setDraft] = useState(String(shown));
  const [typing, setTyping] = useState(false);
  useEffect(() => {
    if (!typing) setDraft(String(shown));
  }, [shown, typing]);
  const fill = ((value * scale - min) / (max - min)) * 100;
  const set = (v: number) => onChange(Math.min(max, Math.max(min, v)) / scale);
  const id = useId();
  const control = (
    <div className="cmp-slider">
      <input
        id={id}
        className="cmp-range"
        type="range"
        min={min}
        max={max}
        step={step}
        value={value * scale}
        aria-label={ariaLabel ?? label}
        style={{ "--fill": `${Math.min(100, Math.max(0, fill))}%` } as CSSProperties}
        onChange={(e) => set(Number(e.target.value))}
      />
      <label className="cmp-num">
        <input
          type="text"
          inputMode="decimal"
          value={draft}
          aria-label={`${ariaLabel ?? label ?? "value"}, typed`}
          onFocus={(e) => {
            setTyping(true);
            e.currentTarget.select();
          }}
          onBlur={() => {
            setTyping(false);
            const n = Number(draft);
            if (Number.isFinite(n) && draft.trim() !== "") set(n);
          }}
          onChange={(e) => {
            setDraft(e.target.value);
            const n = Number(e.target.value);
            // Only what is already in range is applied while typing: clamping "5" on the way to
            // "50" would snap the value to the minimum and move the design under your hands.
            // Leaving the box (onBlur, above) clamps whatever is left there.
            if (Number.isFinite(n) && e.target.value.trim() !== "" && n >= min && n <= max) set(n);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
            if (e.key === "ArrowUp" || e.key === "ArrowDown") {
              e.preventDefault();
              const d = (e.key === "ArrowUp" ? 1 : -1) * step * (e.shiftKey ? 10 : 1);
              set(round(shown + d, step));
            }
          }}
        />
        {unit && <span className="cmp-num-unit">{unit}</span>}
      </label>
    </div>
  );
  return label ? <Field label={label}>{control}</Field> : control;
}

/** A row of buttons, one of them chosen. */
export function Segmented<T extends string | number>({
  options,
  value,
  onChange,
  label,
  small,
}: {
  options: { value: T; label: ReactNode; title?: string }[];
  value: T;
  onChange: (v: T) => void;
  label: string;
  small?: boolean;
}) {
  return (
    <div className={small ? "cmp-seg is-small" : "cmp-seg"} role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button
          key={String(o.value)}
          type="button"
          role="radio"
          aria-checked={o.value === value}
          data-tip={o.title}
          className={o.value === value ? "cmp-seg-btn is-on" : "cmp-seg-btn"}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

/** An on/off switch with its label. */
export function Toggle({ label, on, onChange, hint }: { label: string; on: boolean; onChange: (on: boolean) => void; hint?: string }) {
  return (
    <button type="button" role="switch" aria-checked={on} className="cmp-toggle" onClick={() => onChange(!on)} data-tip={hint}>
      <span className="cmp-toggle-label">{label}</span>
      <span className={on ? "switch is-on" : "switch"} aria-hidden="true">
        <span className="knob" />
      </span>
    </button>
  );
}

/** A small square button with an icon and a tooltip. */
export function IconButton({
  label,
  onClick,
  children,
  disabled,
  active,
  className,
}: {
  label: string;
  onClick: () => void;
  children: ReactNode;
  disabled?: boolean;
  active?: boolean;
  className?: string;
}) {
  return (
    <button
      type="button"
      className={`cmp-icon-btn${active ? " is-on" : ""}${className ? ` ${className}` : ""}`}
      aria-label={label}
      aria-pressed={active}
      data-tip={label}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
