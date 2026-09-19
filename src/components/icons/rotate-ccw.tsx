import type { Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const VARIANTS: Variants = {
  normal: { rotate: "0deg" },
  animate: { rotate: "-50deg" },
};

/** `rotate-ccw` from lucide-animated. */
export function RotateCcwIcon({ size = 15, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <m.svg
      ref={ref}
      {...svgProps(size, className)}
      animate={controls}
      transition={{ type: "spring", stiffness: 250, damping: 25 }}
      variants={VARIANTS}
    >
      <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
      <path d="M3 3v5h5" />
    </m.svg>
  );
}
