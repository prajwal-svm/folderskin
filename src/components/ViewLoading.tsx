import { LoaderIcon } from "./icons/loader";
import { useT } from "../i18n";

/**
 * What a view shows while its code is still on its way (the app loads it ahead once the library
 * is on show, src/lib/warmUp.ts, so this is rarely seen): a spinner where the view will be. With
 * `overlay`, over the whole window, for a dialog.
 */
export function ViewLoading({ overlay = false }: { overlay?: boolean }) {
  const t = useT();
  return (
    <div className={overlay ? "view-loading is-overlay" : "view-loading"} role="status" aria-label={t("common.loading")}>
      <LoaderIcon size={22} />
    </div>
  );
}
