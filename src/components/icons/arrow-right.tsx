import { svgProps } from "./trigger";

/** Lucide's `arrow-right`, static; the button around it slides it along with CSS. */
export function ArrowRightIcon({ size = 14 }: { size?: number }) {
  return (
    <svg {...svgProps(size)}>
      <path d="M5 12h14" />
      <path d="m12 5 7 7-7 7" />
    </svg>
  );
}
