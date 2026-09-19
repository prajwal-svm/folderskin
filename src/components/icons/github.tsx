import { useAnimationControls, type Variants } from "motion/react";
import * as m from "motion/react-m";
import { useMemo, useRef } from "react";
import { type IconProps, svgProps, useIconTrigger } from "./trigger";

const BODY_VARIANTS: Variants = {
  normal: { opacity: 1, pathLength: 1, scale: 1, transition: { duration: 0.3 } },
  animate: { opacity: [0, 1], pathLength: [0, 1], scale: [0.9, 1], transition: { duration: 0.4 } },
};

const TAIL_VARIANTS: Variants = {
  normal: { pathLength: 1, rotate: 0, transition: { duration: 0.3 } },
  draw: { pathLength: [0, 1], rotate: 0, transition: { duration: 0.5 } },
  wag: {
    pathLength: 1,
    rotate: [0, -15, 15, -10, 10, -5, 5],
    transition: { duration: 2.5, ease: "easeInOut", repeat: Number.POSITIVE_INFINITY },
  },
};

/**
 * `github` from lucide-animated: the cat draws itself, then its tail wags while the control is
 * hovered. Leaving before the tail is drawn doesn't start the wag afterwards.
 */
export function GithubIcon({ size = 18, className, playOnMount }: IconProps) {
  const body = useAnimationControls();
  const tail = useAnimationControls();
  const playing = useRef(false);
  const [play, reset] = useMemo(
    () =>
      [
        () => {
          playing.current = true;
          void body.start("animate");
          void tail.start("draw").then(() => {
            if (playing.current) void tail.start("wag");
          });
        },
        () => {
          playing.current = false;
          void body.start("normal");
          void tail.start("normal");
        },
      ] as const,
    [body, tail],
  );
  const ref = useIconTrigger(play, reset, playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      <m.path
        animate={body}
        d="M15 22v-4a4.8 4.8 0 0 0-1-3.5c3 0 6-2 6-5.5.08-1.25-.27-2.48-1-3.5.28-1.15.28-2.35 0-3.5 0 0-1 0-3 1.5-2.64-.5-5.36-.5-8 0C6 2 5 2 5 2c-.3 1.15-.3 2.35 0 3.5A5.403 5.403 0 0 0 4 9c0 3.5 3 5.5 6 5.5-.39.49-.68 1.05-.85 1.65-.17.6-.22 1.23-.15 1.85v4"
        initial="normal"
        variants={BODY_VARIANTS}
      />
      <m.path animate={tail} d="M9 18c-4.51 2-5-2-7-2" initial="normal" variants={TAIL_VARIANTS} />
    </svg>
  );
}
