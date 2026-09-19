import type { Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const BULLET_VARIANTS: Variants = {
  normal: { opacity: 1 },
  animate: (i: number) => ({
    opacity: [0, 1],
    transition: { delay: i * 0.08, duration: 0.2 },
  }),
};

const LINE_VARIANTS: Variants = {
  normal: { pathLength: 1 },
  animate: (i: number) => ({
    pathLength: [0, 1],
    transition: { delay: i * 0.08 + 0.05, duration: 0.3, ease: "easeOut" },
  }),
};

const ROWS = [5, 12, 19];

/** Lucide's `list`, animated here: each row's bullet appears and its line draws, top to bottom. */
export function ListIcon({ size = 18, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      {ROWS.map((y, i) => (
        <m.path key={`b${y}`} animate={controls} custom={i} d={`M3 ${y}h.01`} variants={BULLET_VARIANTS} />
      ))}
      {ROWS.map((y, i) => (
        <m.path key={`l${y}`} animate={controls} custom={i} d={`M8 ${y}h13`} variants={LINE_VARIANTS} />
      ))}
    </svg>
  );
}
