import { useRef, useState, type KeyboardEvent } from "react";
import { refRole, type ChatRef, type RefRole } from "../../state/chats";
import { Popover } from "../composer/Popover";
import { TickIcon, XIcon } from "../icons/composer";
import { useT } from "../../i18n";

const ROLES: RefRole[] = ["subject", "style", "palette"];

/**
 * A reference picture in the prompt, and what it's for: the subject to paint (as a picture is
 * unless it says otherwise), a style to match, or colours to use. The picture opens its choices;
 * one that isn't the subject says what it is on the chip. A picture the model can't take is
 * marked and left out, never silently dropped.
 */
export function RefChip({
  picture: r,
  unused,
  unusedTip,
  onRole,
  onRemove,
}: {
  picture: ChatRef;
  /** The model can't take this one. */
  unused: boolean;
  unusedTip: string;
  onRole: (role: RefRole) => void;
  onRemove: () => void;
}) {
  const t = useT();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const role = refRole(r);
  const roleName = t(`ai.refRole.${role}`);

  // The arrow keys go from choice to choice, as in any group of them.
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const options = [...(list.current?.querySelectorAll<HTMLButtonElement>("[role='radio']") ?? [])];
    const at = options.indexOf(document.activeElement as HTMLButtonElement);
    options[(at + (e.key === "ArrowDown" ? 1 : -1) + options.length) % options.length]?.focus();
    e.preventDefault();
  };

  return (
    <span className={unused ? "composer-ref is-unused" : "composer-ref"} data-tip={unused ? unusedTip : undefined}>
      <button
        type="button"
        className="composer-ref-pick"
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        aria-label={t("ai.refRole.label", { name: r.name, role: roleName })}
        data-tip={unused ? undefined : t(`ai.refRole.${role}Tip`)}
        onMouseDown={(e) => e.preventDefault()}
        onClick={(e) => setAnchor(anchor ? null : e.currentTarget)}
      >
        <img src={r.thumb} alt="" draggable={false} />
        <span className="composer-ref-name">{r.name}</span>
        {role !== "subject" && <span className="composer-ref-role">{roleName}</span>}
      </button>
      <button type="button" className="composer-ref-x" aria-label={t("ai.prompt.removeRefLabel", { name: r.name })} data-tip={t("ai.prompt.removeRef")} onClick={onRemove}>
        <XIcon size={12} />
      </button>
      {anchor && (
        <Popover anchor={anchor} onClose={() => setAnchor(null)} width={280} label={t("ai.refRole.menuLabel")} className="ref-role-pop">
          <p className="ref-role-title">{t("ai.refRole.title")}</p>
          <div className="ref-role-list" role="radiogroup" aria-label={t("ai.refRole.menuLabel")} ref={list} onKeyDown={onKey}>
            {ROLES.map((option) => {
              const on = option === role;
              return (
                <button
                  key={option}
                  type="button"
                  role="radio"
                  aria-checked={on}
                  className={on ? "shape-option is-on" : "shape-option"}
                  onClick={() => {
                    setAnchor(null);
                    onRole(option);
                  }}
                >
                  <span className="shape-option-text">
                    <span className="shape-option-name">{t(`ai.refRole.${option}`)}</span>
                    <span className="shape-option-note">{t(`ai.refRole.${option}Note`)}</span>
                  </span>
                  {on && <TickIcon size={15} />}
                </button>
              );
            })}
          </div>
        </Popover>
      )}
    </span>
  );
}
