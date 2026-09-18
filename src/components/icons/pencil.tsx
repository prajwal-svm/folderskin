import type { Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const VARIANTS: Variants = {
  normal: { rotate: 0 },
  animate: {
    rotate: [0, -12, 8, -4, 0],
    transition: { duration: 0.6, ease: "easeInOut" },
  },
};

/** Lucide's `pencil`. It scribbles when its button is hovered. */
export function PencilIcon({ size = 14, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <m.svg ref={ref} {...svgProps(size, className)} animate={controls} variants={VARIANTS}>
      <path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z" />
      <path d="m15 5 4 4" />
    </m.svg>
  );
}
