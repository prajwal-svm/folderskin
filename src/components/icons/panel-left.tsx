import { svgProps } from "./trigger";

/** Lucide's `panel-left-close`, static: folds the sidebar to a rail. */
export function PanelLeftCloseIcon({ size = 16 }: { size?: number }) {
  return (
    <svg {...svgProps(size)}>
      <rect width="18" height="18" x="3" y="3" rx="2" />
      <path d="M9 3v18" />
      <path d="m16 15-3-3 3-3" />
    </svg>
  );
}

/** Lucide's `panel-left-open`, static: opens the folded sidebar again. */
export function PanelLeftOpenIcon({ size = 16 }: { size?: number }) {
  return (
    <svg {...svgProps(size)}>
      <rect width="18" height="18" x="3" y="3" rx="2" />
      <path d="M9 3v18" />
      <path d="m14 9 3 3-3 3" />
    </svg>
  );
}
