import { CheckIcon } from "./icons/check";

/** The "done / saved" mark: a white tick in a filled green circle, drawn in when it appears. */
export function OkBadge({ size = 16, playOnMount, label }: { size?: number; playOnMount?: boolean; label?: string }) {
  return (
    <span
      className="ok-badge"
      style={{ width: size, height: size }}
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      data-tip={label}
    >
      <CheckIcon size={Math.round(size * 0.66)} playOnMount={playOnMount} />
    </span>
  );
}
