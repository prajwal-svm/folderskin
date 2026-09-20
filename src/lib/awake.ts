/**
 * Whether anything on screen is worth animating: the window is visible and in front.
 *
 * A window sitting behind another still runs its animations, and a repainting gradient or a
 * marching outline costs as much in the background as in front — on this app, tens of percent of
 * a core. `data-awake="false"` on <html> pauses every animation until the window comes back
 * (app.css); paused, not cancelled, so nothing jumps when it does.
 */
export function watchAwake(): () => void {
  const set = () => {
    const awake = document.visibilityState === "visible" && document.hasFocus();
    document.documentElement.dataset.awake = awake ? "true" : "false";
  };
  set();
  window.addEventListener("focus", set);
  window.addEventListener("blur", set);
  document.addEventListener("visibilitychange", set);
  return () => {
    window.removeEventListener("focus", set);
    window.removeEventListener("blur", set);
    document.removeEventListener("visibilitychange", set);
  };
}
