import type { Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const VARIANTS: Variants = {
  normal: { x: 0, y: 0 },
  animate: { x: [0, 0, -3, 0], y: [0, -4, 0, 0] },
};

/** `search` from lucide-animated. */
export function SearchIcon({ size = 16, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <m.svg
      ref={ref}
      {...svgProps(size, className)}
      animate={controls}
      transition={{ duration: 1, bounce: 0.3 }}
      variants={VARIANTS}
    >
      <circle cx="11" cy="11" r="8" />
      <path d="m21 21-4.3-4.3" />
    </m.svg>
  );
}
