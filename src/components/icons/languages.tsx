import type { Variants } from "motion/react";
import * as m from "motion/react-m";
import { type IconProps, svgProps, useIconControls } from "./trigger";

/** The two scripts lean towards each other and back, as if one turned into the other. */
const VARIANTS: Variants = {
  normal: { x: 0, y: 0 },
  animate: (towards: number) => ({
    x: [0, towards * 1.2, 0],
    y: [0, towards * -1.2, 0],
    transition: { duration: 0.5, ease: "easeInOut" },
  }),
};

/** Lucide's `languages` (文A), animated by FolderSkin: the character and the letter nod to each other. */
export function LanguagesIcon({ size = 18, className, playOnMount }: IconProps) {
  const { ref, controls } = useIconControls(playOnMount);
  return (
    <svg ref={ref} {...svgProps(size, className)}>
      <m.g animate={controls} custom={1} variants={VARIANTS}>
        <path d="m5 8 6 6" />
        <path d="m4 14 6-6 2-3" />
        <path d="M2 5h12" />
        <path d="M7 2h1" />
      </m.g>
      <m.g animate={controls} custom={-1} variants={VARIANTS}>
        <path d="m22 22-5-10-5 10" />
        <path d="M14 18h6" />
      </m.g>
    </svg>
  );
}
