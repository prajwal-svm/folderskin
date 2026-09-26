import { BadgeCheckIcon } from "./icons/badge-check";
import { HardDriveIcon } from "./icons/hard-drive";
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

/** Marks a pack of drives, whose skins are drawn on the drive they go on: a tag's shape with a drive in it. */
export function DrivesBadge() {
  const t = useT();
  return (
    <span className="tag-chip is-drives">
      <HardDriveIcon size={12} />
      {t("community.pack.drives")}
    </span>
  );
}
