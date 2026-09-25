import { BadgeCheckIcon } from "./icons/badge-check";

/** Marks a pack the maintainer lists as official: a tag's shape in the accent colour, first in its row of tags. */
export function OfficialBadge() {
  return (
    <span className="tag-chip is-official">
      <BadgeCheckIcon size={12} />
      Official
    </span>
  );
}
