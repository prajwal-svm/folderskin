import { svgProps } from "./trigger";

/** Lucide's `loader-circle`, turned by the `.spin` keyframes in app.css rather than by Motion. */
export function LoaderIcon({ size = 14 }: { size?: number }) {
  return (
    <svg {...svgProps(size, "spin")}>
      <path d="M21 12a9 9 0 1 1-6.219-8.56" />
    </svg>
  );
}
