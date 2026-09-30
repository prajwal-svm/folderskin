import { OkBadge } from "./OkBadge";
import { InfoIcon } from "./icons/info";
import { useT } from "../i18n";
import { formatNumber } from "../i18n/format";

/**
 * How many of a pack's skins are in the library while the pack isn't: the tick and "3/40", with
 * the words in its tip. An added pack says so on its own.
 */
export function HaveChip({ have, total }: { have: number; total: number }) {
  const t = useT();
  const label = t("community.inLibrary.have", { have, count: total });
  return (
    <span className="have-chip" role="img" aria-label={label} data-tip={label}>
      <OkBadge size={14} />
      {formatNumber(have)}/{formatNumber(total)}
    </span>
  );
}

/**
 * An ⓘ that holds the longer words in its tip, for the pointer and the keyboard alike. Pressing it
 * does nothing: there is nothing more to it than the tip.
 */
export function InfoTip({ text, className }: { text: string; className?: string }) {
  return (
    <span className={className ? `info-tip ${className}` : "info-tip"} tabIndex={0} role="img" aria-label={text} data-tip={text}>
      <InfoIcon size={15} />
    </span>
  );
}
