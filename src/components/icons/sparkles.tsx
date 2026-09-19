import { useAnimationControls, type Variants } from "motion/react";
import * as m from "motion/react-m";
import { useMemo } from "react";
import { type IconProps, svgProps, useIconTrigger } from "./trigger";

const SPARKLE_VARIANTS: Variants = {
  initial: { y: 0, fill: "none" },
  hover: {
    y: [0, -1, 0, 0],
    fill: "currentColor",
    transition: { duration: 1, bounce: 0.3 },
  },
};

const STAR_VARIANTS: Variants = {
  initial: { opacity: 1, x: 0, y: 0 },
  blink: () => ({
    opacity: [0, 1, 0, 0, 0, 0, 1],
    transition: { duration: 2, type: "spring", stiffness: 70, damping: 10, mass: 0.4 },
  }),
};

const STAR_PATHS = ["M20 3v4", "M22 5h-4", "M4 17v2", "M5 18H3"];

/** `sparkles` from lucide-animated: the big sparkle fills and hops, then the small ones blink. */
export function SparklesIcon({ size = 20, className, playOnMount }: IconProps) {
  const sparkle = useAnimationControls();
  const stars = useAnimationControls();
  const [play, reset] = useMemo(
    () =>
      [
        () => {
          void sparkle.start("hover");
          void stars.start("blink", { delay: 1 });
        },
        () => {
          void sparkle.start("initial");
          void stars.start("initial");
        },
      ] as const,
    [sparkle, stars],
  );
  const ref = useIconTrigger(play, reset, playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      <m.path
        animate={sparkle}
        d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"
        variants={SPARKLE_VARIANTS}
      />
      {STAR_PATHS.map((d) => (
        <m.path key={d} animate={stars} d={d} variants={STAR_VARIANTS} />
      ))}
    </svg>
  );
}
