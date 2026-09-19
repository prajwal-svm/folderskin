import type { Transition, Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const PATHS = [
  "M21.54 15H17a2 2 0 0 0-2 2v4.54",
  "M7 3.34V5a3 3 0 0 0 3 3a2 2 0 0 1 2 2c0 1.1.9 2 2 2a2 2 0 0 0 2-2c0-1.1.9-2 2-2h3.17",
  "M11 21.95V18a2 2 0 0 0-2-2a2 2 0 0 1-2-2v-1a2 2 0 0 0-2-2H2.05",
];

const PATH_TRANSITION: Transition = { duration: 0.7, delay: 0.5, opacity: { delay: 0.5 } };

const PATH_VARIANTS: Variants = {
  normal: { pathLength: 1, opacity: 1, pathOffset: 0 },
  animate: { pathLength: [0, 1], opacity: [0, 1], pathOffset: [1, 0] },
};

const CIRCLE_TRANSITION: Transition = { duration: 0.3, delay: 0.1, opacity: { delay: 0.15 } };

const CIRCLE_VARIANTS: Variants = {
  normal: { pathLength: 1, opacity: 1 },
  animate: { pathLength: [0, 1], opacity: [0, 1] },
};

/** `earth` from lucide-animated. */
export function EarthIcon({ size = 20, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      {PATHS.map((d) => (
        <m.path key={d} animate={controls} d={d} transition={PATH_TRANSITION} variants={PATH_VARIANTS} />
      ))}
      <m.circle
        animate={controls}
        cx="12"
        cy="12"
        r="10"
        transition={CIRCLE_TRANSITION}
        variants={CIRCLE_VARIANTS}
      />
    </svg>
  );
}
