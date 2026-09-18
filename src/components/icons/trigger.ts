/**
 * Shared plumbing for the animated icons in this folder.
 *
 * The icons come from lucide-animated (see LICENSES.md) with two changes. They use `m` from
 * `motion/react-m` instead of `motion`, so only the animation features loaded by the
 * `LazyMotion` in `main.tsx` reach the bundle. And they play while the control around them is
 * hovered or keyboard-focused, instead of listening on a wrapper div: that div is invalid inside
 * a button and only reacts to the glyph's own few pixels.
 */
import { useAnimationControls } from "motion/react";
import { useLayoutEffect, useMemo, useRef } from "react";

export interface IconProps {
  size?: number;
  className?: string;
  /** Play once when the icon mounts, for icons that confirm something just happened. */
  playOnMount?: boolean;
}

/** Elements whose hover or keyboard focus plays the icons inside them. */
const HOST = "button, a, label, [role='button'], [role='tab'], [data-icon-host]";

/**
 * Calls `play` while the icon's host is hovered or keyboard-focused and `reset` when it is
 * left. Returns the ref to put on the icon's `<svg>`. `play` and `reset` must be stable.
 *
 * A layout effect, so `playOnMount` starts before the first paint instead of showing the
 * finished icon for a frame and then restarting it.
 */
export function useIconTrigger(play: () => void, reset: () => void, playOnMount = false) {
  const ref = useRef<SVGSVGElement>(null);

  useLayoutEffect(() => {
    const svg = ref.current;
    const host = svg?.closest<HTMLElement>(HOST) ?? svg?.parentElement;
    if (!host) return;
    const focus = (e: FocusEvent) => {
      if (e.target instanceof Element && e.target.matches(":focus-visible")) play();
    };
    host.addEventListener("pointerenter", play);
    host.addEventListener("pointerleave", reset);
    host.addEventListener("focusin", focus);
    host.addEventListener("focusout", reset);
    if (playOnMount) play();
    return () => {
      host.removeEventListener("pointerenter", play);
      host.removeEventListener("pointerleave", reset);
      host.removeEventListener("focusin", focus);
      host.removeEventListener("focusout", reset);
    };
  }, [play, reset, playOnMount]);

  return ref;
}

/** The common case: one set of controls moving between `normal` and `animate` variants. */
export function useIconControls(playOnMount?: boolean) {
  const controls = useAnimationControls();
  const [play, reset] = useMemo(
    () => [() => void controls.start("animate"), () => void controls.start("normal")] as const,
    [controls],
  );
  const ref = useIconTrigger(play, reset, playOnMount);
  return { ref, controls };
}

/** Attributes every icon's `<svg>` shares, matching Lucide's 24 px grid and 2 px stroke. */
export const svgProps = (size: number, className?: string) => ({
  className,
  width: size,
  height: size,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 2,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
});
