import { svgProps } from "./trigger";

/** Lucide's `info`, static. */
export function InfoIcon({ size = 14 }: { size?: number }) {
  return (
    <svg {...svgProps(size)}>
      <circle cx="12" cy="12" r="10" /><path d="M12 16v-4" /><path d="M12 8h.01" />
    </svg>
  );
}
