import type { Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const ARROW_VARIANTS: Variants = {
  normal: { y: 0 },
  animate: {
    y: 2,
    transition: { type: "spring", stiffness: 200, damping: 10, mass: 1 },
  },
};

/** `download` from lucide-animated. */
export function DownloadIcon({ size = 18, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
      <m.g animate={controls} variants={ARROW_VARIANTS}>
        <polyline points="7 10 12 15 17 10" />
        <line x1="12" x2="12" y1="15" y2="3" />
      </m.g>
    </svg>
  );
}
