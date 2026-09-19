import type { Transition, Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const SPRING: Transition = { type: "spring", stiffness: 500, damping: 30 };

const LID_VARIANTS: Variants = {
  normal: { y: 0 },
  animate: { y: -1.1 },
};

const CAN_VARIANTS: Variants = {
  normal: { d: "M19 8v12c0 1-1 2-2 2H7c-1 0-2-1-2-2V8" },
  animate: { d: "M19 9v12c0 1-1 2-2 2H7c-1 0-2-1-2-2V9" },
};

const LINE_VARIANTS: Variants = {
  normal: { y1: 11, y2: 17 },
  animate: { y1: 11.5, y2: 17.5 },
};

/** `delete` from lucide-animated: the lid lifts off the can. */
export function DeleteIcon({ size = 16, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      <m.g animate={controls} initial="normal" transition={SPRING} variants={LID_VARIANTS}>
        <path d="M3 6h18" />
        <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
      </m.g>
      <m.path animate={controls} d="M19 8v12c0 1-1 2-2 2H7c-1 0-2-1-2-2V8" initial="normal" transition={SPRING} variants={CAN_VARIANTS} />
      <m.line animate={controls} initial="normal" transition={SPRING} variants={LINE_VARIANTS} x1="10" x2="10" y1="11" y2="17" />
      <m.line animate={controls} initial="normal" transition={SPRING} variants={LINE_VARIANTS} x1="14" x2="14" y1="11" y2="17" />
    </svg>
  );
}
