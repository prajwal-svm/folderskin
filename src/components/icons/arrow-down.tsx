import type { Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const PATH_VARIANTS: Variants = {
  normal: { d: "m19 12-7 7-7-7", translateY: 0 },
  animate: {
    d: "m19 12-7 7-7-7",
    translateY: [0, -3, 0],
    transition: { duration: 0.4 },
  },
};

const SECOND_PATH_VARIANTS: Variants = {
  normal: { d: "M12 5v14" },
  animate: {
    d: ["M12 5v14", "M12 5v9", "M12 5v14"],
    transition: { duration: 0.4 },
  },
};

/** `arrow-down` from lucide-animated. */
export function ArrowDownIcon({ size = 14, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      <m.path animate={controls} d="m19 12-7 7-7-7" variants={PATH_VARIANTS} />
      <m.path animate={controls} d="M12 5v14" variants={SECOND_PATH_VARIANTS} />
    </svg>
  );
}
