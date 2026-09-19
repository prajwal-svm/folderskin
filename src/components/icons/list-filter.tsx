import type { Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

/** Each line as it rests and as it narrows, from the widest down. */
const LINES = [
  ["M3 6h18", "M6 6h12"],
  ["M7 12h10", "M9 12h6"],
  ["M10 18h4", "M11 18h2"],
] as const;

const variants = (rest: string, narrow: string): Variants => ({
  normal: { d: rest },
  animate: (i: number) => ({
    d: [rest, narrow, rest],
    transition: { delay: i * 0.07, duration: 0.5, ease: "easeInOut" },
  }),
});

/** Lucide's `list-filter`, animated here: the lines narrow one after another, like a funnel. */
export function ListFilterIcon({ size = 18, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      {LINES.map(([rest, narrow], i) => (
        <m.path key={rest} animate={controls} custom={i} initial="normal" d={rest} variants={variants(rest, narrow)} />
      ))}
    </svg>
  );
}
