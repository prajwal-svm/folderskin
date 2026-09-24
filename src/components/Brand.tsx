import { Fragment, type ReactNode } from "react";

const NAME = "FolderSkin";

/** The app's name as it's written on screen, wherever it appears: bold, "Skin" in FolderSkin's blue. */
export function Brand() {
  return (
    <b className="brand-word">
      Folder<span className="brand-accent">Skin</span>
    </b>
  );
}

/**
 * Words handed to a dialog, a setting, a toast or a tooltip, with the app's name in them written
 * as the name is. Anything that isn't plain text is shown as it is.
 */
export function branded(text: ReactNode): ReactNode {
  if (typeof text !== "string" || !text.includes(NAME)) return text;
  return text.split(NAME).map((part, i) => (
    <Fragment key={i}>
      {i > 0 && <Brand />}
      {part}
    </Fragment>
  ));
}
