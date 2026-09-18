import type { Transition, Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const VARIANTS: Variants = {
  normal: { rotate: 0 },
  animate: { rotate: [0, -10, 10, -5, 5, 0] },
};

const TRANSITION: Transition = { duration: 1.2, ease: "easeInOut" };

/** `moon` from lucide-animated. */
export function MoonIcon({ size = 18, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <m.svg ref={ref} {...svgProps(size, className)} animate={controls} transition={TRANSITION} variants={VARIANTS}>
      <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" />
    </m.svg>
  );
}
