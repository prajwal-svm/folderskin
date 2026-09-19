import type { Variants } from "motion/react";
import * as m from "motion/react-m";
import { useEffect, useRef } from "react";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const VARIANTS: Variants = {
  normal: { scale: 1 },
  animate: { scale: [1, 1.08, 1] },
};

/**
 * Lucide's `star`, which lucide-animated does not have, animated like its `heart`: a pulse on
 * hover, and again whenever it fills.
 */
export function StarIcon({ size = 14, className, playOnMount, filled = false }: IconProps & { filled?: boolean }) {
  const { ref, controls } = useIconControls(playOnMount);
  const wasFilled = useRef(filled);

  useEffect(() => {
    if (filled && !wasFilled.current) void controls.start("animate");
    wasFilled.current = filled;
  }, [filled, controls]);

  return (
    <m.svg
      ref={ref}
      {...svgProps(size, className)}
      fill={filled ? "currentColor" : "none"}
      animate={controls}
      transition={{ duration: 0.45, repeat: 2 }}
      variants={VARIANTS}
    >
      <path d="M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z" />
    </m.svg>
  );
}
