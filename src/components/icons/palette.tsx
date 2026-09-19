import type { Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

const DOT_VARIANTS: Variants = {
  normal: { scale: 1 },
  animate: (i: number) => ({
    scale: [1, 1.9, 1],
    transition: { duration: 0.42, delay: i * 0.08, ease: "easeOut" },
  }),
};

const DOTS = [
  [13.5, 6.5],
  [17.5, 10.5],
  [8.5, 7.5],
  [6.5, 12.5],
] as const;

/** Lucide's `palette`, animated by FolderSkin: its paint dots pop one after another. */
export function PaletteIcon({ size = 18, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      <path d="M12 22a1 1 0 0 1 0-20 10 9 0 0 1 10 9 5 5 0 0 1-5 5h-2.25a1.75 1.75 0 0 0-1.4 2.8l.3.4a1.75 1.75 0 0 1-1.4 2.8z" />
      {DOTS.map(([cx, cy], i) => (
        <m.circle
          key={i}
          cx={cx}
          cy={cy}
          r=".5"
          fill="currentColor"
          custom={i}
          animate={controls}
          variants={DOT_VARIANTS}
          style={{ transformOrigin: `${cx}px ${cy}px` }}
        />
      ))}
    </svg>
  );
}
