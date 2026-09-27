/**
 * The code of the views the app loads lazily, so launch doesn't wait for them: the AI view,
 * "Design your own" and the subfolder chooser. Each is fetched once the library is on show
 * (src/lib/warmUp.ts), or the first time its view opens if that's sooner, and `React.lazy` takes
 * it from the same import.
 */
export const loadStudio = () => import("./studio/Studio");
export const loadComposer = () => import("./composer/Composer");
export const loadChooser = () => import("./SubfolderChooser");
