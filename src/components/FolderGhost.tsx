/**
 * The empty folder, drawn as a dashed blueprint of FolderSkin's own template. The paths are
 * the compositor's shapes on its 1024 canvas (printed by
 * `cargo run -p folderskin-core --example outline_svg`), so a real folder rendered into the
 * same box lines up with this outline exactly.
 */
const BACK =
  "M61 58.5C61 46.3 70.8 36.5 83 36.5L410.8 36.5C417.8 36.5 423.9 41 426 47.7L432 66.3C437.8 84.6 454.8 97 473.9 97L949 97C974.4 97 995 117.6 995 143L995 927.5C995 952.9 974.4 973.5 949 973.5L75 973.5C49.6 973.5 29 952.9 29 927.5L29 127.8C29 117.8 35.5 109 45 105.9C54.5 102.9 61 94 61 84Z";
const PAPER =
  "M74.5 143.3C74.5 136.7 79.9 131.3 86.5 131.3L937.5 131.3C944.1 131.3 949.5 136.7 949.5 143.3L949.5 973.5L74.5 973.5Z";
const FRONT =
  "M15 215.5C15 185.1 39.6 160.5 70 160.5L954 160.5C984.4 160.5 1009 185.1 1009 215.5L1009 918.5C1009 948.9 984.4 973.5 954 973.5L70 973.5C39.6 973.5 15 948.9 15 918.5Z";

export function FolderGhost({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 1024 1024" aria-hidden="true">
      <path className="ghost-back" d={BACK} />
      <path className="ghost-paper" d={PAPER} />
      <path className="ghost-front" d={FRONT} />
    </svg>
  );
}
