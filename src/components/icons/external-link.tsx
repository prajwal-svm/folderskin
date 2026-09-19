import type { Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const ARROW_VARIANTS: Variants = {
  normal: { scale: 1, translateX: 0, translateY: 0 },
  animate: {
    scale: [1, 0.92, 1],
    translateX: [0, 2, 0],
    translateY: [0, -2, 0],
    originX: 1,
    originY: 0,
    transition: { duration: 0.5, ease: "easeInOut" },
  },
};

/** `external-link` from lucide-animated. */
export function ExternalLinkIcon({ size = 15, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
      <m.g animate={controls} variants={ARROW_VARIANTS}>
        <path d="M15 3h6v6" />
        <path d="M10 14 21 3" />
      </m.g>
    </svg>
  );
}
