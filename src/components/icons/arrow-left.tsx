import { svgProps } from "./trigger";

/** Lucide's `arrow-left`, static; the stage's nudge animates it with CSS. */
export function ArrowLeftIcon({ size = 14 }: { size?: number }) {
  return (
    <svg {...svgProps(size)}>
      <path d="m12 19-7-7 7-7" />
      <path d="M19 12H5" />
    </svg>
  );
}
