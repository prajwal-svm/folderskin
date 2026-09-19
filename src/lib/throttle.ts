/**
 * Hands values on at most once every `ms`, the first straight away and then the latest of each
 * wait. A run over a folder's tree reports every folder, and a revert gets through about a
 * thousand a second: far more than anyone can read, and each would be a render of its own. A
 * timer rather than animation frames, which stop while the window is hidden or covered.
 */
export function throttle<T>(send: (value: T) => void, ms = 50): { push: (value: T) => void; cancel: () => void } {
  let last = -Infinity;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let waiting: { value: T } | null = null;
  const flush = () => {
    timer = undefined;
    if (!waiting) return;
    const { value } = waiting;
    waiting = null;
    last = Date.now();
    send(value);
  };
  return {
    push(value) {
      waiting = { value };
      if (timer !== undefined) return;
      const wait = last + ms - Date.now();
      if (wait <= 0) flush();
      else timer = setTimeout(flush, wait);
    },
    /** Drops whatever is still waiting: the run is over, and its result says the rest. */
    cancel() {
      clearTimeout(timer);
      timer = undefined;
      waiting = null;
    },
  };
}
