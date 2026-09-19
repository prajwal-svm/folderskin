import type { Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const SUN_VARIANTS: Variants = {
  normal: { y: 0 },
  animate: { y: [0, -1.5, 0], transition: { duration: 0.6, ease: "easeInOut" } },
};

const HILL_VARIANTS: Variants = {
  normal: { pathLength: 1, opacity: 1 },
  animate: { pathLength: [0, 1], opacity: [0, 1], transition: { duration: 0.5, delay: 0.1 } },
};

/**
 * Lucide's `image`, which lucide-animated does not have, animated in the same style as its
 * `earth`: the sun bobs and the hill redraws.
 */
export function ImageIcon({ size = 18, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      <rect width="18" height="18" x="3" y="3" rx="2" ry="2" />
      <m.circle animate={controls} cx="9" cy="9" r="2" variants={SUN_VARIANTS} />
      <m.path animate={controls} d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21" variants={HILL_VARIANTS} />
    </svg>
  );
}
