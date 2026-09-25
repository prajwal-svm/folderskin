import type { KeyboardEvent } from "react";
import type { FolderStyle } from "../composer/parts";
import { useT } from "../i18n";

/** The Apple and Windows marks (Simple Icons, CC0), only to name the two folders. */
function AppleMark() {
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M12.152 6.896c-.948 0-2.415-1.078-3.96-1.04-2.04.027-3.91 1.183-4.961 3.014-2.117 3.675-.546 9.103 1.519 12.09 1.013 1.454 2.208 3.09 3.792 3.039 1.52-.065 2.09-.987 3.935-.987 1.831 0 2.35.987 3.96.948 1.637-.026 2.676-1.48 3.676-2.948 1.156-1.688 1.636-3.325 1.662-3.415-.039-.013-3.182-1.221-3.22-4.857-.026-3.04 2.48-4.494 2.597-4.559-1.429-2.09-3.623-2.324-4.39-2.376-2-.156-3.675 1.09-4.61 1.09zM15.53 3.83c.843-1.012 1.4-2.427 1.245-3.83-1.207.052-2.662.805-3.532 1.818-.78.896-1.454 2.338-1.273 3.714 1.338.104 2.715-.688 3.559-1.701" />
    </svg>
  );
}

function WindowsMark() {
  return (
    <svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M0 0h11.377v11.372H0zm12.623 0H24v11.372H12.623zM0 12.623h11.377V24H0zm12.623 0H24V24H12.623" />
    </svg>
  );
}

const LOOKS: { value: FolderStyle; Mark: typeof AppleMark }[] = [
  { value: "mac", Mark: AppleMark },
  { value: "windows", Mark: WindowsMark },
];

/**
 * Mac or Windows: which folder a skin goes on. The same switch in the folder panel, where it
 * chooses for the whole app, and under the composer's canvas, where it chooses for one design.
 * Arrow keys move the choice, as radio buttons do.
 */
export function LookSwitch({ value, onChange, label, className }: { value: FolderStyle; onChange: (look: FolderStyle) => void; label?: string; className?: string }) {
  const t = useT();
  const onKey = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(e.key)) return;
    e.preventDefault();
    const next = value === "mac" ? "windows" : "mac";
    onChange(next);
    (e.currentTarget.parentElement?.querySelector(`[data-look="${next}"]`) as HTMLButtonElement | null)?.focus();
  };
  return (
    <div className={["cmp-seg is-small look-switch", className].filter(Boolean).join(" ")} role="radiogroup" aria-label={label ?? t("folder.look.label")}>
      {LOOKS.map(({ value: v, Mark }) => (
        <button
          key={v}
          type="button"
          role="radio"
          aria-checked={v === value}
          tabIndex={v === value ? 0 : -1}
          data-look={v}
          data-tip={t(`folder.look.tip.${v}`)}
          className={v === value ? "cmp-seg-btn is-on" : "cmp-seg-btn"}
          onClick={() => onChange(v)}
          onKeyDown={onKey}
        >
          <Mark />
          {t(`folder.look.name.${v}`)}
        </button>
      ))}
    </div>
  );
}
