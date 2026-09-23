import { type CSSProperties, useEffect, useState } from "react";

/** Previews already decoded this session, so a card drawn again shows its picture at once. */
const ready = new Set<string>();
/** How long a card has to stay before its preview is asked for: one that only flashes past in
 *  a fast scroll asks for nothing. */
const SETTLE_MS = 90;

/** Loads a preview into the browser's cache and resolves once it can be drawn. */
function load(src: string, image = new Image()): Promise<void> {
  image.decoding = "async";
  image.src = src;
  return image.decode().then(() => {
    ready.add(src);
  });
}

/** Starts loading a pack's preview, so it's there when the pack is shown. */
export function prefetchPreview(src: string): void {
  if (src && !ready.has(src)) load(src).catch(() => {});
}

/**
 * A pack's preview: its first few skins as folders, from the address the list gives. As a strip
 * it shows the picture as it is; as a `grid` it cuts it into its folders and lays them out two
 * by two. Until it has loaded it shimmers, and it is only asked for once the card has settled on
 * screen.
 */
export function PackPreview({ src, count, grid }: { src: string; count: number; grid: boolean }) {
  const [shown, setShown] = useState(() => ready.has(src));
  useEffect(() => {
    if (ready.has(src)) {
      setShown(true);
      return;
    }
    setShown(false);
    if (!src) return;
    let live = true;
    const image = new Image();
    const timer = setTimeout(() => {
      load(src, image)
        .then(() => live && setShown(true))
        .catch(() => {});
    }, SETTLE_MS);
    return () => {
      live = false;
      clearTimeout(timer);
      // Scrolled away before it came: dropping the source lets the browser give up on it.
      if (!ready.has(src)) image.src = "";
    };
  }, [src]);

  if (grid) {
    // The strip holds up to four folders side by side, one per cell here.
    const cells = Math.max(1, Math.min(count, 4));
    return (
      <span className="pack-preview">
        {shown ? (
          <span className="pack-quad" style={{ "--strip": `url("${src}")` } as CSSProperties}>
            {Array.from({ length: cells }, (_, i) => (
              <span
                key={i}
                style={{
                  backgroundSize: `${cells * 100}% 100%`,
                  backgroundPositionX: cells === 1 ? "0%" : `${(i / (cells - 1)) * 100}%`,
                }}
              />
            ))}
          </span>
        ) : (
          <span className="pack-preview-blank" />
        )}
      </span>
    );
  }
  return <span className="pack-preview">{shown ? <img src={src} alt="" draggable={false} /> : <span className="pack-preview-blank" />}</span>;
}
