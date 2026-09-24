import { useId } from "react";
import { useLook } from "../state/look";

/**
 * The folder template as SVG: FolderSkin's own, or the folder Windows draws when skins go on
 * that one (state/look.ts). The paths are the compositor's shapes on its 1024 canvas
 * (printed by `cargo run -p folderskin-core --example outline_svg`), so a real folder rendered
 * into the same box lines up with this outline exactly.
 *
 * `tone="mac"` fills it with the default folder's colours: macOS's blue (the same gradient
 * `compositor::default_folder_artwork` uses), or Windows' yellow. `tone="neutral"` is a quiet grey
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

const COLOURS = {
  mac: { back: ["#64b9f1", "#3f9cdd"], front: ["#8ad0f8", "#52abe7"] },
  windows: { back: ["#fdbe18", "#f2ab0c"], front: ["#ffe18a", "#ffd256"] },
};

export function FolderGhost({
  className,
  tone = "neutral",
  layer = "both",
}: {
  className?: string;
  tone?: "mac" | "neutral";
  layer?: "fill" | "line" | "both";
}) {
  const id = useId().replace(/[^a-zA-Z0-9]/g, "");
  const fill = layer !== "line";
  const line = layer !== "fill";
  const mac = tone === "mac";
  const windows = useLook() === "windows";
  const colours = COLOURS[windows ? "windows" : "mac"];
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
      <path className="ghost-back" d={windows ? WIN_BACK : BACK} fill={mac && fill ? `url(#${id}back)` : undefined} />
      {!windows && <path className="ghost-paper" d={PAPER} />}
      <path className="ghost-front" d={windows ? WIN_FRONT : FRONT} fill={mac && fill ? `url(#${id}front)` : undefined} />
    </svg>
  );
}
