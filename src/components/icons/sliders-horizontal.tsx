import type { Transition } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const TRANSITION: Transition = { type: "spring", stiffness: 100, damping: 12, mass: 0.4 };

/** Each line with the attributes it rests at (`normal`) and slides to (`animate`). */
const LINES = [
  { x1: 21, x2: 14, y1: 4, y2: 4, normal: { x2: 14 }, animate: { x2: 10 } },
  { x1: 10, x2: 3, y1: 4, y2: 4, normal: { x1: 10 }, animate: { x1: 5 } },
  { x1: 21, x2: 12, y1: 12, y2: 12, normal: { x2: 12 }, animate: { x2: 18 } },
  { x1: 8, x2: 3, y1: 12, y2: 12, normal: { x1: 8 }, animate: { x1: 13 } },
  { x1: 3, x2: 12, y1: 20, y2: 20, normal: { x2: 12 }, animate: { x2: 4 } },
  { x1: 16, x2: 21, y1: 20, y2: 20, normal: { x1: 16 }, animate: { x1: 8 } },
  { x1: 14, x2: 14, y1: 2, y2: 6, normal: { x1: 14, x2: 14 }, animate: { x1: 9, x2: 9 } },
  { x1: 8, x2: 8, y1: 10, y2: 14, normal: { x1: 8, x2: 8 }, animate: { x1: 14, x2: 14 } },
  { x1: 16, x2: 16, y1: 18, y2: 22, normal: { x1: 16, x2: 16 }, animate: { x1: 8, x2: 8 } },
];

/** `sliders-horizontal` from lucide-animated. */
export function SlidersHorizontalIcon({ size = 18, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      {LINES.map(({ normal, animate, ...line }) => (
        <m.line
          key={`${line.x1},${line.y1},${line.x2},${line.y2}`}
          animate={controls}
          // Without a starting variant motion reads each line's ends back as undefined on the
          // first hover and writes that into the SVG before animating.
          initial="normal"
          transition={TRANSITION}
          variants={{ normal, animate }}
          {...line}
        />
      ))}
    </svg>
  );
}
