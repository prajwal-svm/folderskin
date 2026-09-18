import { useCallback, useRef, useState } from "react";

export type ToastTone = "info" | "ok" | "danger";

export type Toast = {
  id: number;
  text: string;
  tone: ToastTone;
  action?: { label: string; run: () => void };
  leaving?: boolean;
};

/** How long a toast stays; ones with an action (Undo) stay longer so there's time to use it. */
const PLAIN_MS = 2800;
const ACTION_MS = 5200;
const LEAVE_MS = 220;

/**
 * Short confirmations for things that happen away from where the user is looking: a picture
 * saved to Yours, a skin removed (with Undo), an icon put back. At most three at a time.
 */
export function useToasts() {
  const [items, setItems] = useState<Toast[]>([]);
  const seq = useRef(0);

  const dismiss = useCallback((id: number) => {
    setItems((xs) => xs.map((x) => (x.id === id ? { ...x, leaving: true } : x)));
    window.setTimeout(() => setItems((xs) => xs.filter((x) => x.id !== id)), LEAVE_MS);
  }, []);

  const push = useCallback(
    (text: string, opts: { tone?: ToastTone; action?: Toast["action"] } = {}) => {
      const id = ++seq.current;
      setItems((xs) => [...xs.filter((x) => !x.leaving).slice(-2), { id, text, tone: opts.tone ?? "info", action: opts.action }]);
      window.setTimeout(() => dismiss(id), opts.action ? ACTION_MS : PLAIN_MS);
      return id;
    },
    [dismiss],
  );

  return { items, push, dismiss };
}
