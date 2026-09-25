import { BadgeCheckIcon } from "./icons/badge-check";
import { useT } from "../i18n";

/** Marks a pack the maintainer lists as official: a tag's shape in the accent colour, first in its row of tags. */
export function OfficialBadge() {
  const t = useT();
  return (
    <span className="tag-chip is-official">
      <BadgeCheckIcon size={12} />
      {t("common.official")}
    </span>
  );
}
