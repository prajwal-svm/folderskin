import type { Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const CHECK_VARIANTS: Variants = {
  normal: { pathLength: 1, opacity: 1, transition: { duration: 0.3 } },
  animate: {
    pathLength: [0, 1],
    opacity: [0, 1],
    transition: {
      pathLength: { duration: 0.4, ease: "easeInOut" },
      opacity: { duration: 0.4, ease: "easeInOut" },
    },
  },
};

/** `monitor-check` from lucide-animated: the check on the screen draws itself again. */
export function MonitorCheckIcon({ size = 18, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      <rect height="14" rx="2" width="20" x="2" y="3" />
      <path d="M12 17v4" />
      <path d="M8 21h8" />
      <m.path
        animate={controls}
        d="m9 10 2 2 4-4"
        initial="normal"
        style={{ transformOrigin: "center" }}
        variants={CHECK_VARIANTS}
      />
    </svg>
  );
}
