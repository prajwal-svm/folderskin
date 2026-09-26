import { svgProps } from "./trigger";

/** Lucide's `panel-right-close`, static: closes the folder panel on the right. */
export function PanelRightCloseIcon({ size = 16 }: { size?: number }) {
  return (
    <svg {...svgProps(size)}>
      <rect width="18" height="18" x="3" y="3" rx="2" />
      <path d="M15 3v18" />
      <path d="m8 9 3 3-3 3" />
    </svg>
  );
}

/** Lucide's `panel-right-open`, static: opens the folder panel again. */
export function PanelRightOpenIcon({ size = 16 }: { size?: number }) {
  return (
    <svg {...svgProps(size)}>
      <rect width="18" height="18" x="3" y="3" rx="2" />
      <path d="M15 3v18" />
      <path d="m10 15-3-3 3-3" />
    </svg>
  );
}
