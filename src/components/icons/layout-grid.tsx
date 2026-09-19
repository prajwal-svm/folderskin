import type { Transition, Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const TRANSITION: Transition = { duration: 0.8, ease: "easeInOut", times: [0, 0.4, 0.6, 1] };

/** Each square slides one cell clockwise and back. */
const move = (x: number, y: number): Variants => ({
  normal: { translateX: 0, translateY: 0 },
  animate: {
    translateX: [0, x, x, 0],
    translateY: [0, y, y, 0],
    transition: TRANSITION,
  },
});

const RECTS = [
  { x: 3, y: 3, variants: move(11, 0) },
  { x: 14, y: 3, variants: move(0, 11) },
  { x: 14, y: 14, variants: move(-11, 0) },
  { x: 3, y: 14, variants: move(0, -11) },
];

/** `layout-grid` from lucide-animated. */
export function LayoutGridIcon({ size = 20, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      {RECTS.map(({ x, y, variants }) => (
        <m.rect
          key={`${x},${y}`}
          animate={controls}
          height="7"
          initial="normal"
          rx="1"
          variants={variants}
          width="7"
          x={x}
          y={y}
        />
      ))}
    </svg>
  );
}
