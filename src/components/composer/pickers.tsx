import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { Assets, ctx2d } from "../../composer/assets";
import { makePattern, makeShape, PATTERNS, SHAPES, type PatternKind, type ShapeKind } from "../../composer/doc";
import { EMOJI, searchEmoji } from "../../composer/emoji";
import { CUSTOM, FONTS } from "../../composer/fonts";
import { drawPattern } from "../../composer/patterns";
import { evenOdd, traceShape } from "../../composer/shapes";
import { graphemes } from "../../composer/text";
import { CheckIcon } from "../icons/check";

/** One emoji out of what someone typed or pasted: the first thing that looks like one. */
function firstEmoji(s: string): string | null {
  for (const g of graphemes(s.trim())) if (/\p{Extended_Pictographic}|\p{Regional_Indicator}/u.test(g)) return g;
  return null;
}

/** Emoji by group or by search, plus a field any emoji can be typed or pasted into. */
export function EmojiPicker({ onPick }: { onPick: (char: string) => void }) {
  const [query, setQuery] = useState("");
  const [group, setGroup] = useState(EMOJI[0].id);
  const found = useMemo(() => searchEmoji(query), [query]);
  const typed = firstEmoji(query);
  const list = query.trim() ? (typed && !found.includes(typed) ? [typed, ...found] : found) : (EMOJI.find((g) => g.id === group)?.items.map(([c]) => c) ?? []);
  return (
    <div className="cmp-emoji">
      <input
        className="cmp-search"
        placeholder="Search, or paste any emoji"
        value={query}
        autoFocus
        spellCheck={false}
        aria-label="search emoji"
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && list[0]) onPick(list[0]);
        }}
      />
      {!query.trim() && (
        <div className="cmp-emoji-tabs" role="tablist" aria-label="emoji groups">
          {EMOJI.map((g) => (
            <button
              key={g.id}
              type="button"
              role="tab"
              aria-selected={g.id === group}
              data-tip={g.label}
              aria-label={g.label}
              className={g.id === group ? "cmp-emoji-tab is-on" : "cmp-emoji-tab"}
              onClick={() => setGroup(g.id)}
            >
              {g.icon}
            </button>
          ))}
        </div>
      )}
      <div className="cmp-emoji-grid">
        {list.map((c) => (
          <button key={c} type="button" className="cmp-emoji-btn" aria-label={c} onClick={() => onPick(c)}>
            {c}
          </button>
        ))}
        {list.length === 0 && <p className="cmp-empty-note">No emoji for "{query.trim()}". Paste one from your keyboard's emoji picker.</p>}
      </div>
    </div>
  );
}

/** The fonts, each shown in itself, and a field for any other font on this computer. */
export function FontPicker({ value, onPick }: { value: string; onPick: (id: string) => void }) {
  const [other, setOther] = useState(value.startsWith(CUSTOM) ? value.slice(CUSTOM.length) : "");
  return (
    <div className="cmp-fonts">
      {FONTS.map((f) => (
        <button key={f.id} type="button" className={f.id === value ? "cmp-font is-on" : "cmp-font"} onClick={() => onPick(f.id)}>
          <span className="cmp-font-sample" style={{ fontFamily: f.stack }}>
            {f.label}
          </span>
          {f.id === value && <CheckIcon size={14} />}
        </button>
      ))}
      <form
        className="cmp-font-other"
        onSubmit={(e) => {
          e.preventDefault();
          if (other.trim()) onPick(`${CUSTOM}${other.trim()}`);
        }}
      >
        <input
          className="cmp-search"
          placeholder="Another font you have, by name"
          value={other}
          spellCheck={false}
          aria-label="another font"
          onChange={(e) => setOther(e.target.value)}
          style={other.trim() ? { fontFamily: `"${other.trim().replace(/"/g, "")}", system-ui` } : undefined}
        />
        <button type="submit" className="btn btn-secondary cmp-font-use" disabled={!other.trim()}>
          Use
        </button>
      </form>
    </div>
  );
}

const shared = new Assets();

/** A small drawing made with the composer's own drawing code. */
function Mini({ draw, size = 44, label }: { draw: (ctx: CanvasRenderingContext2D, px: number) => void; size?: number; label: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const run = useRef(draw);
  run.current = draw;
  useLayoutEffect(() => {
    const c = ref.current;
    if (!c) return;
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    const px = Math.round(size * dpr);
    c.width = px;
    c.height = px;
    const ctx = ctx2d(c);
    ctx.clearRect(0, 0, px, px);
    run.current(ctx, px);
  });
  return <canvas ref={ref} className="cmp-mini" style={{ width: size, height: size }} role="img" aria-label={label} />;
}

export function ShapeGrid({ value, onPick, color = "currentColor" }: { value?: ShapeKind; onPick: (s: ShapeKind) => void; color?: string }) {
  const ink = color === "currentColor" ? "#3a86ff" : color;
  return (
    <div className="cmp-grid">
      {SHAPES.map((s) => (
        <button key={s.id} type="button" className={s.id === value ? "cmp-grid-btn is-on" : "cmp-grid-btn"} data-tip={s.label} onClick={() => onPick(s.id)}>
          <Mini
            label={s.label}
            size={36}
            draw={(ctx, px) => {
              const layer = makeShape(s.id, 0, 0, ink);
              const k = (px * 0.8) / Math.max(layer.w, layer.h);
              ctx.translate(px / 2, px / 2);
              ctx.scale(k, k);
              ctx.beginPath();
              traceShape(ctx, layer);
              ctx.fillStyle = ink;
              ctx.fill(evenOdd(layer) ? "evenodd" : "nonzero");
            }}
          />
          <span className="cmp-grid-label">{s.label}</span>
        </button>
      ))}
    </div>
  );
}

/** The patterns, each drawn in the accent colour so they read on a light or dark panel. */
export function PatternGrid({ value, onPick }: { value?: PatternKind; onPick: (p: PatternKind) => void }) {
  return (
    <div className="cmp-grid">
      {PATTERNS.map((p) => (
        <button key={p.id} type="button" className={p.id === value ? "cmp-grid-btn is-on" : "cmp-grid-btn"} data-tip={p.label} onClick={() => onPick(p.id)}>
          <Mini
            label={p.label}
            size={44}
            draw={(ctx, px) => {
              const layer = { ...makePattern(p.id, "#3a86ff", "#3a86ff26"), seed: 7 };
              // A corner of the canvas, so a few repeats fit in the swatch.
              ctx.scale(px / 220, px / 220);
              ctx.beginPath();
              ctx.rect(0, 0, 220, 220);
              ctx.clip();
              if (p.id === "grain") {
                ctx.fillStyle = "#3a86ff";
                ctx.fillRect(0, 0, 220, 220);
                layer.color = "#ffffff";
                layer.background = "#00000000";
                layer.scale = 1;
              }
              drawPattern(ctx, layer, (seed, c) => shared.grain(seed, c));
            }}
          />
          <span className="cmp-grid-label">{p.label}</span>
        </button>
      ))}
    </div>
  );
}
