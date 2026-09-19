import type { Transition, Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const TRANSITION: Transition = { duration: 0.4, ease: "easeInOut" };

const LID_VARIANTS: Variants = {
  normal: { scaleY: 1, opacity: 1 },
  animate: { scaleY: [1, 0.1, 1], opacity: [1, 0.3, 1] },
};

const PUPIL_VARIANTS: Variants = {
  normal: { scale: 1, opacity: 1 },
  animate: { scale: [1, 0.3, 1], opacity: [1, 0.3, 1] },
};

/** `eye` from lucide-animated: it blinks. */
export function EyeIcon({ size = 16, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      <m.path
        animate={controls}
        d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"
        style={{ originY: "50%" }}
        transition={TRANSITION}
        variants={LID_VARIANTS}
      />
      <m.circle animate={controls} cx="12" cy="12" r="3" transition={TRANSITION} variants={PUPIL_VARIANTS} />
    </svg>
  );
}
