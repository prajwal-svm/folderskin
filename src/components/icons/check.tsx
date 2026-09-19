import type { Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const PATH_VARIANTS: Variants = {
  normal: {
    opacity: 1,
    pathLength: 1,
    scale: 1,
    transition: { duration: 0.3, opacity: { duration: 0.1 } },
  },
  animate: {
    opacity: [0, 1],
    pathLength: [0, 1],
    scale: [0.5, 1],
    transition: { duration: 0.4, opacity: { duration: 0.1 } },
  },
};

/** `check` from lucide-animated. With `playOnMount` it draws itself as it appears. */
export function CheckIcon({ size = 14, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      <m.path animate={controls} d="M4 12 9 17L20 6" initial="normal" variants={PATH_VARIANTS} />
    </svg>
  );
}
