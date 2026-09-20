# FolderSkin website

The website shares the app colors, Manrope font, theme preference, logo, and README screenshots. It uses static HTML and Vite without a frontend framework.

## Local preview

From the repository root, install dependencies with `pnpm install --frozen-lockfile`.

Start the preview with `pnpm site:dev`. Open `http://localhost:4173/folderskin/`.

Use `pnpm site:test` to test system detection and installer selection. Use `pnpm site:build` to create `website/dist`.

Use `pnpm site:preview` to serve the production build at `http://localhost:4174/folderskin/`.

## Downloads

The main button recommends an installer for the detected operating system. Mobile devices and unknown systems get a link to all downloads.

macOS uses the universal installer for Apple silicon and Intel. Windows uses the x64 installer. Windows ARM users see an emulation note.

Linux uses architecture hints when the browser provides them. Platforms with multiple installers show a selector. A platform with one installer shows its name. Linux includes AppImage, Debian, and RPM packages.

A download click reads the latest stable release from GitHub. The browser downloads its matching installer directly from GitHub.

If that request fails, the page offers GitHub releases. It does not silently download an older installer.

The small release snapshot supplies version and size labels before GitHub responds. Update it with `pnpm site:refresh`.

The GitHub star count refreshes when the page opens. The snapshot supplies a fallback when GitHub cannot respond.

## Appearance and assets

The website follows the system theme until the visitor chooses light or dark. The footer selector restores system mode.

The build copies selected app assets into the ignored folders under `website/public`. Edit the original assets in the app or README folders.

The website preserves image dimensions to prevent layout shifts. Images below the hero load as the visitor scrolls. Fonts come from the same site.

The Linux icon uses [Tux from Simple Icons](https://github.com/simple-icons/simple-icons/blob/develop/icons/linux.svg).

## GitHub Pages

Set the Pages source to GitHub Actions in the repository configuration.

The Website workflow tests and builds pull requests. After this change reaches main, website changes deploy automatically.

Downloads resolve each new public release without a website rebuild. A manual workflow run on main also refreshes the fallback snapshot.

The deployment URL is `https://prajwal-svm.github.io/folderskin/`. Set `SITE_BASE=/` only when a custom domain serves the website at its root.
