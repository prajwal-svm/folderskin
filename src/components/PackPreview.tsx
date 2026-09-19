import { type CSSProperties, useEffect, useState } from "react";
import { api } from "../lib/tauri";

/** Previews downloaded this session, so showing a pack again doesn't load it again. */
const previews = new Map<string, string>();
/** Previews on their way, so asking twice while one downloads makes one request. */
const pending = new Map<string, Promise<string>>();

/** Forgets every preview, for Refresh: the next ones come from GitHub again. */
export function clearPreviews(): void {
  previews.clear();
  pending.clear();
}

/** A pack's preview as a data URL, from this session's copy unless `fresh`. */
function loadPreview(id: string, fresh: boolean): Promise<string> {
  const have = previews.get(id);
  if (have && !fresh) return Promise.resolve(have);
  const waiting = pending.get(id);
  if (waiting && !fresh) return waiting;
  const request = api
    .communityPreview(id, fresh)
    .then((url) => {
      previews.set(id, url);
      return url;
    })
    .finally(() => {
      if (pending.get(id) === request) pending.delete(id);
    });
  pending.set(id, request);
  return request;
}

/** Starts downloading a pack's preview, so it's there when the pack is shown. */
export function prefetchPreview(id: string): void {
  loadPreview(id, false).catch(() => {});
}

/**
 * A pack's preview: its first few skins as folders, loaded once per session. As a strip it shows
 * the picture as it is; as a `grid` it cuts it into its folders and lays them out two by two.
 */
export function PackPreview({ id, count, grid, fresh = false }: { id: string; count: number; grid: boolean; fresh?: boolean }) {
  const [src, setSrc] = useState(() => previews.get(id) ?? null);
  useEffect(() => {
    if (src) return;
    let live = true;
    loadPreview(id, fresh)
      .then((url) => {
        if (live) setSrc(url);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [id, src, fresh]);
  if (grid) {
    // The strip holds up to four folders side by side, one per cell here.
    const shown = Math.max(1, Math.min(count, 4));
    return (
      <span className="pack-preview">
        {src ? (
          <span className="pack-quad" style={{ "--strip": `url("${src}")` } as CSSProperties}>
            {Array.from({ length: shown }, (_, i) => (
              <span
                key={i}
                style={{
                  backgroundSize: `${shown * 100}% 100%`,
                  backgroundPositionX: shown === 1 ? "0%" : `${(i / (shown - 1)) * 100}%`,
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
  return <span className="pack-preview">{src ? <img src={src} alt="" draggable={false} /> : <span className="pack-preview-blank" />}</span>;
}
