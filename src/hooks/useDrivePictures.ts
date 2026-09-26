import { useCallback, useEffect, useReducer } from "react";
import { api, type Skin } from "../lib/tauri";
import { drawnOnDrive, type Drive } from "../lib/drives";

/**
 * Each skin's picture on the drive picked, for the library's cards and the stage: artwork drawn
 * onto the drive's shape, a finished drive or folder as it was drawn. The app draws a picture the
 * first time a card or the stage loads it (drive_thumbs.rs), so the cards off screen cost nothing.
 * With no drive picked, a skin's picture is its own thumbnail.
 */
export function useDrivePictures(drive: Drive | null): (skin: Skin) => string {
  // The browser preview draws its pictures late, and says when each is ready.
  const [drawn, redraw] = useReducer((n: number) => n + 1, 0);
  useEffect(() => api.watchDriveThumbnails(redraw), []);
  return useCallback(
    (skin: Skin) => {
      if (!drive || !drawnOnDrive(skin)) return skin.thumbnail;
      return api.driveThumbnail(drive, skin.id) ?? drive.plain;
    },
    // `drawn` changes the function, so what shows it asks again once a late picture is ready.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [drive, drawn],
  );
}
