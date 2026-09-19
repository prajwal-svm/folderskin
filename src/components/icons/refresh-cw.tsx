import type { Transition, Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const VARIANTS: Variants = {
  normal: { rotate: "0deg" },
  animate: { rotate: "50deg" },
};

const TRANSITION: Transition = { type: "spring", stiffness: 250, damping: 25 };

/** `refresh-cw` from lucide-animated: the arrows turn forward. */
export function RefreshCwIcon({ size = 16, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <m.svg ref={ref} {...svgProps(size, className)} animate={controls} transition={TRANSITION} variants={VARIANTS}>
      <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" />
      <path d="M21 3v5h-5" />
      <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" />
      <path d="M8 16H3v5" />
    </m.svg>
  );
}
