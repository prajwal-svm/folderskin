import { useEffect, useState } from "react";
import { type Palette, readPalette } from "../lib/palette";
import type { Skin } from "../lib/tauri";

const KEY = "folderskin.palettes.v1";
/** How many pictures are read at once before the window gets a turn to paint. */
const BATCH = 6;

function load(): [string, Palette][] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(parsed) ? (parsed as [string, Palette][]) : [];
  } catch {
    return [];
  }
}

/** Palettes read so far, by skin id. A skin's id comes from its pixels, so a palette never goes stale. */
const known = new Map<string, Palette>(load());

/** Keeps the palettes of the skins still in the library, in this browser only. */
function save(skins: Skin[]) {
  const ids = new Set(skins.map((s) => s.id));
  for (const id of known.keys()) if (!ids.has(id)) known.delete(id);
  try {
    localStorage.setItem(KEY, JSON.stringify([...known]));
  } catch {
    // They're read again next time; nothing else depends on them.
  }
}

/**
 * Each skin's colours and brightness, for the gallery's filters. Skins seen before are known at
 * once; new ones are read from their pictures a few at a time in the background.
 */
export function usePalettes(skins: Skin[]): { palettes: ReadonlyMap<string, Palette>; reading: boolean } {
  const [palettes, setPalettes] = useState<ReadonlyMap<string, Palette>>(() => new Map(known));
  const [reading, setReading] = useState(false);

  useEffect(() => {
    const todo = skins.filter((s) => !known.has(s.id));
    if (!todo.length) {
      setReading(false);
      return;
    }
    let cancelled = false;
    setReading(true);
    void (async () => {
      for (let i = 0; i < todo.length && !cancelled; i += BATCH) {
        const read = await Promise.all(
          todo.slice(i, i + BATCH).map((s) =>
            readPalette(s.thumbnail).then(
              (p) => [s.id, p] as const,
              () => null,
            ),
          ),
        );
        for (const entry of read) if (entry) known.set(entry[0], entry[1]);
        if (cancelled) break;
        setPalettes(new Map(known));
        await new Promise((r) => setTimeout(r, 0));
      }
      if (!cancelled) {
        save(skins);
        setReading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [skins]);

  return { palettes, reading };
}
