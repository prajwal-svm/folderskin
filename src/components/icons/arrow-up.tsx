import { svgProps } from "./trigger";

/** Lucide's `arrow-up`, static. */
export function ArrowUpIcon({ size = 16 }: { size?: number }) {
  return (
    <svg {...svgProps(size)}>
      <path d="m5 12 7-7 7 7" /><path d="M12 19V5" />
    </svg>
  );
}
