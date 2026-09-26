import { useId } from "react";
import type { FolderStyle } from "../composer/parts";
import { useLook } from "../state/look";

/**
 * The folder template as SVG: FolderSkin's own, or the folder Windows or Linux draws when skins
 * go on that one (state/look.ts). The paths are the compositor's shapes on its 1024 canvas
 * (printed by `cargo run -p folderskin-core --example outline_svg`), so a real folder rendered
 * into the same box lines up with this outline exactly.
 *
 * `tone="mac"` fills it with the default folder's colours: macOS's blue (the same gradient
 * `compositor::default_folder_artwork` uses), Windows' yellow or Linux's deeper blue
 * (`compositor::default_folder_artwork_in`). `tone="neutral"` is a quiet grey
 * placeholder. `layer` draws the filled folder, its marching dashed outline, or both, so the
 * outline can sit on top of something shown inside the folder.
 */
const BACK =
  "M61 58.5C61 46.3 70.8 36.5 83 36.5L410.8 36.5C417.8 36.5 423.9 41 426 47.7L432 66.3C437.8 84.6 454.8 97 473.9 97L949 97C974.4 97 995 117.6 995 143L995 927.5C995 952.9 974.4 973.5 949 973.5L75 973.5C49.6 973.5 29 952.9 29 927.5L29 127.8C29 117.8 35.5 109 45 105.9C54.5 102.9 61 94 61 84Z";
const PAPER =
  "M74.5 143.3C74.5 136.7 79.9 131.3 86.5 131.3L937.5 131.3C944.1 131.3 949.5 136.7 949.5 143.3L949.5 973.5L74.5 973.5Z";
const FRONT =
  "M15 215.5C15 185.1 39.6 160.5 70 160.5L954 160.5C984.4 160.5 1009 185.1 1009 215.5L1009 918.5C1009 948.9 984.4 973.5 954 973.5L70 973.5C39.6 973.5 15 948.9 15 918.5Z";
/** Windows' folder: no paper between its panels. */
const WIN_BACK =
  "M64 176C64 153.9 81.9 136 104 136L356 136C410 136 410 232 464 232L924 232C943.9 232 960 248.1 960 268L960 804C960 823.9 943.9 840 924 840L100 840C80.1 840 64 823.9 64 804Z";
const WIN_FRONT =
  "M64 332C64 312.1 80.1 296 100 296L376 296C420 296 420 248 464 248L924 248C943.9 248 960 264.1 960 284L960 804C960 823.9 943.9 840 924 840L100 840C80.1 840 64 823.9 64 804Z";
/** The Linux folder: a tab that slopes down to its body, and no paper either. */
const LIN_BACK =
  "M72 174C72 155.2 87.2 140 106 140L360.8 140C368 140 375 142.6 380.5 147.4L439.4 198.6C444.9 203.4 451.9 206 459.1 206L912 206C934.1 206 952 223.9 952 246L952 832C952 854.1 934.1 872 912 872L112 872C89.9 872 72 854.1 72 832Z";
const LIN_FRONT =
  "M72 316C72 293.9 89.9 276 112 276L912 276C934.1 276 952 293.9 952 316L952 832C952 854.1 934.1 872 912 872L112 872C89.9 872 72 854.1 72 832Z";
const PATHS: Record<FolderStyle, { back: string; front: string }> = {
  mac: { back: BACK, front: FRONT },
  windows: { back: WIN_BACK, front: WIN_FRONT },
  linux: { back: LIN_BACK, front: LIN_FRONT },
};

const COLOURS: Record<FolderStyle, { back: [string, string]; front: [string, string] }> = {
  mac: { back: ["#64b9f1", "#3f9cdd"], front: ["#8ad0f8", "#52abe7"] },
  windows: { back: ["#fdbe18", "#f2ab0c"], front: ["#ffe18a", "#ffd256"] },
  linux: { back: ["#5e93d2", "#2f6fc4"], front: ["#6caaf2", "#3a86e4"] },
};

export function FolderGhost({
  className,
  tone = "neutral",
  layer = "both",
  look,
}: {
  className?: string;
  tone?: "mac" | "neutral";
  layer?: "fill" | "line" | "both";
  /** The folder to draw, when it isn't the one skins go on now: a picture made for Windows' folder shows Windows' folder. */
  look?: FolderStyle;
}) {
  const id = useId().replace(/[^a-zA-Z0-9]/g, "");
  const fill = layer !== "line";
  const line = layer !== "fill";
  const mac = tone === "mac";
  const current = useLook();
  const which = look ?? current;
  const colours = COLOURS[which];
  const paths = PATHS[which];
  const cls = ["ghost", `ghost-${tone}`, fill ? "ghost-fill" : "", line ? "ghost-line" : "", className]
    .filter(Boolean)
    .join(" ");
  return (
    <svg className={cls} viewBox="0 0 1024 1024" aria-hidden="true">
      {mac && fill && (
        <defs>
          <linearGradient id={`${id}back`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor={colours.back[0]} />
            <stop offset="1" stopColor={colours.back[1]} />
          </linearGradient>
          <linearGradient id={`${id}front`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor={colours.front[0]} />
            <stop offset="1" stopColor={colours.front[1]} />
          </linearGradient>
        </defs>
      )}
      <path className="ghost-back" d={paths.back} fill={mac && fill ? `url(#${id}back)` : undefined} />
      {which === "mac" && <path className="ghost-paper" d={PAPER} />}
      <path className="ghost-front" d={paths.front} fill={mac && fill ? `url(#${id}front)` : undefined} />
    </svg>
  );
}

/** A free icon's placeholder, for a picture on its way that has no folder around it: a soft square, and the object in it. */
const ICON_TILE = "M232 152L792 152C836.2 152 872 187.8 872 232L872 792C872 836.2 836.2 872 792 872L232 872C187.8 872 152 836.2 152 792L152 232C152 187.8 187.8 152 232 152Z";
const ICON_OBJECT = "M512 312C622.5 312 712 401.5 712 512C712 622.5 622.5 712 512 712C401.5 712 312 622.5 312 512C312 401.5 401.5 312 512 312Z";

export function IconGhost({ className }: { className?: string }) {
  return (
    <svg className={["ghost", "ghost-neutral", "ghost-fill", "ghost-line", className].filter(Boolean).join(" ")} viewBox="0 0 1024 1024" aria-hidden="true">
      <path className="ghost-back" d={ICON_TILE} />
      <path className="ghost-front" d={ICON_OBJECT} />
    </svg>
  );
}
