<div align="center">

<img src="public/app-icon.png" alt="FolderSkin logo" width="112" height="112" />

# FolderSkin

[![Download for macOS](https://img.shields.io/badge/Download_for_macOS-111827?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/prajwal-svm/folderskin/releases/latest)
[![Download for Windows](https://img.shields.io/badge/Download_for_Windows-3A86FF?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA0NDggNTEyIj48cGF0aCBmaWxsPSIjZmZmIiBkPSJtMCA5My43bDE4My42LTI1LjN2MTc3LjRIMHptMCAzMjQuNmwxODMuNiAyNS4zVjI2OC40SDB6bTIwMy44IDI4TDQ0OCA0ODBWMjY4LjRIMjAzLjh6bTAtMzgwLjZ2MTgwLjFINDQ4VjMyeiIvPjwvc3ZnPg==)](https://github.com/prajwal-svm/folderskin/releases/latest)
[![Download for Linux](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest)

[![CI](https://github.com/prajwal-svm/folderskin/actions/workflows/ci.yml/badge.svg)](https://github.com/prajwal-svm/folderskin/actions/workflows/ci.yml) [![Quality gate](https://sonarcloud.io/api/project_badges/measure?project=prajwal-svm_folderskin&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=prajwal-svm_folderskin) [![Coverage](https://sonarcloud.io/api/project_badges/measure?project=prajwal-svm_folderskin&metric=coverage)](https://sonarcloud.io/component_measures?id=prajwal-svm_folderskin&metric=coverage) [![Downloads](https://img.shields.io/github/downloads/prajwal-svm/folderskin/total?color=3A86FF)](https://github.com/prajwal-svm/folderskin/releases) [![Version](https://img.shields.io/github/package-json/v/prajwal-svm/folderskin?label=version&color=3A86FF)](CHANGELOG.md) [![Last commit](https://img.shields.io/github/last-commit/prajwal-svm/folderskin?label=last%20commit&color=3A86FF)](https://github.com/prajwal-svm/folderskin/commits/main) [![License: MIT](https://img.shields.io/badge/license-MIT-3A86FF)](LICENSE) [![Built with Tauri 2](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB?logo=tauri&logoColor=white)](https://tauri.app) [![Community packs welcome](https://img.shields.io/badge/community%20packs-welcome-12b981)](docs/PACKS.md) [![Stars](https://img.shields.io/github/stars/prajwal-svm/folderskin?style=social)](https://github.com/prajwal-svm/folderskin)

**Give any folder a skin.**

FolderSkin turns a picture into a folder icon on macOS, Windows and Linux. Drop a folder on the
window, try skins on it and press apply: a built-in skin, a photo of your own, a pack someone
shared, or a poster-style folder an AI paints for you. Revert puts the system icon back.

<p>
  <a href="https://github.com/prajwal-svm/folderskin/releases/latest"><strong>Download free</strong></a>
  · <a href="docs/PACKS.md">Share a skin pack</a>
  · <a href="#build-from-source">Build from source</a>
</p>

Free · Open source · No account · No tracking

</div>

<div align="center">
  <img src="docs/images/app.png" alt="FolderSkin: the skin library in the middle, and a folder trying on the Sunset skin on the right" width="100%" />
</div>

<details><summary>Dark mode</summary>

![FolderSkin in dark mode](docs/images/app-dark.png)

</details>

## Start here

| If you want to... | Do this |
| --- | --- |
| Give a folder a new look | Drop the folder on the window, click a skin, press **Apply skin** |
| Use a photo of your own | Drop the picture on the window, or press **Add your photo** |
| Have an AI paint one | **Generate with AI**, with your own API key, or the prompt for Grok's or ChatGPT's chat under **No API key?** |
| Get skins other people made | **Community**, then add a pack |
| Share yours | A skin's ⋯ menu → **Share with community** |
| Find a skin again | The tags along the top, ⌘F / Ctrl+F, or the star for **Favourites** |
| Undo it | **Revert** puts the operating system's icon back |

## What stays on your computer

Everything, unless you ask for something that needs the internet. There's no account, no
paywall and no telemetry, and the macOS app is about 10 MB installed.

| Stays on your computer | Goes online only when you ask |
| --- | --- |
| Your folders and the icons FolderSkin writes | An AI request, sent to the provider you picked, with your key |
| Every picture you add and every skin you make | Community, which reads the shared packs from GitHub |
| Your AI keys, in a file only your account can read | |
| Favourites, tags and settings | |

## How to use it

1. Drag a folder onto the folder panel on the right, or click the empty folder to pick one.
2. Click a skin in the library to try it on; the folder panel shows the result straight away.
   The sidebar picks **All skins**, **Yours** or **Favourites**, the tags along the top narrow
   that down, and ⌘F / Ctrl+F searches.
3. Press **Apply skin**. The folder is marked **Applied**, and **Show in Finder** opens it.
4. Press **Revert** to put the operating system's default icon back.

To use your own picture, drop it on the window or press **Add your photo**. It is saved under
**Yours** and stays there until you delete it, which asks first. A finished folder on a flat
magenta background, like the ones the chat prompt below produces, is cut out and used as it is;
any other picture is wrapped onto FolderSkin's folder.

Every skin you add has a ⋯ menu: rename it, give it tags (they become filters along the top),
see how it was made (the AI model and prompt, or the pack and who shared it), share it or delete
it. AI results are tagged with their style, such as `airbrush`, as they arrive.

**Settings**, at the bottom of the sidebar, holds the theme, your AI keys, what sharing fills in
and where your skins are saved. Hover the version badge beside the logo for About.

## The ten built-in skins

![the ten built-in skins](docs/images/skins.png)

| collection | skins |
|---|---|
| `glow` | Aurora, Sunset, Mesh, Ember |
| `grain` | Paper, Denim, Slate |
| `pop` | Halftone, Stripes, Bubbles |

All ten are original procedural art, generated by the repository's own tool from a seeded
random number generator and released under CC0. Nothing here is borrowed from another product.

## Community skins

People share skins and packs of skins on GitHub, free for everyone. Open **Community** to add
one: its skins join your library with their tags. **Classic Art** is a good first pack: sixteen
public-domain paintings, from the Mona Lisa to The Starry Night, each painted onto a folder. To share yours, open a skin's ⋯ menu and
choose **Share with community**, or use **Community → Share your skins** for several. FolderSkin
saves a pack folder that passes the checks, and you drop it on GitHub as a pull request.
[docs/PACKS.md](docs/PACKS.md) has the contract and its limits: 1 to 24 skins a pack, pictures up
to 1024 px and 2 MB, licensed CC0, CC BY 4.0 or MIT.

## Generate a skin with AI

FolderSkin can make a skin from a description, using **your own API key** from a provider you
already use. The key is saved in a private file on your computer that only your account can
read (no keychain password prompts), FolderSkin has no server of its own, and nothing is sent
anywhere until you press Enter. Open **Generate with AI**, describe a scene, pick a style, and
choose **Whole folder** (the model paints the whole folder from FolderSkin's template, like a
poster) or **Just the art** (flat art wrapped onto FolderSkin's folder). Every result is saved to
**Yours** and can be tried on at once.

No key? [docs/PROMPTS.md](docs/PROMPTS.md) has a template and a prompt for Grok's or
ChatGPT's own chat; the app shows the same prompt, filled in, under **No API key?**.

[docs/AI.md](docs/AI.md) covers the providers, where the key is stored, how transparency is
handled for models that cannot return an alpha channel, and what each error message means.

## Add to the built-in skins

A themed set, such as 3D folders rendered with an image model, ships as a **built-in pack**: a
pack folder under `assets/packs/`, the same format as a community pack, embedded at build time.
`folderskin-tools packs make` cuts finished folders out of their magenta background and
compresses every picture to fit; [docs/PACKS.md](docs/PACKS.md#built-in-packs) has the steps.

A single built-in skin is a 1024 × 958 image under 400 KB. The maintainer CLI does the cropping
and the bookkeeping:

```sh
cargo run -p folderskin-tools -- skin add ~/Pictures/dunes.jpg \
  --id dunes --name Dunes --collection grain --focus 0.5,0.45
cargo run -p folderskin-tools -- skin check
pnpm tauri build          # skins are embedded at build time
```

Then look at `assets/previews/dunes.png` and adjust `--focus` until the composition is right.
[docs/SKINS.md](docs/SKINS.md) explains the safe areas — the top 12% of the image lands in the
folder's tab, the front panel keeps the middle ~85% of the height — and
[.claude/skills/folderskin-skins/SKILL.md](.claude/skills/folderskin-skins/SKILL.md) teaches
Claude Code to run the whole loop for you, so "make a skin from this photo" is a reasonable
thing to ask.

## How it works

The Rust core (`crates/folderskin-core`) holds the folder template as vector paths,
cover-fits your image into the back panel and the front panel,
renders the whole thing once at 2048 px with `tiny-skia`, and downsamples to every icon size
with Lanczos3. The webview never draws folder geometry — it shows PNGs the core rendered — so
the gallery thumbnail, the preview and the icon on disk are the same pixels on every
platform. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) has the details.

## Platform notes

| OS | mechanism | files written inside the folder | caveat |
|---|---|---|---|
| macOS | `NSWorkspace.setIcon` | the invisible `Icon\r` file macOS maintains | none; Finder updates immediately |
| Windows | `desktop.ini` + `folderskin.ico`, both hidden + system, folder marked read-only, then `SHChangeNotify` | `desktop.ini`, `folderskin.ico` | Explorer's icon cache may need `F5` |
| Linux | `.directory` for KDE, plus `gio set metadata::custom-icon` for Nautilus, Nemo and Caja | `.directory`, `.folderskin.png` | some tiling and minimal file managers read neither |

Revert removes only what FolderSkin wrote, and is safe to run twice. Cloud-synced folders
(iCloud, OneDrive, Dropbox) will sync the helper files to your other machines. The full
picture, including reverting by hand, is in [docs/PLATFORMS.md](docs/PLATFORMS.md).

## Download

Free · Open source · No account · No tracking

| Platform | Package | Download |
| --- | --- | --- |
| macOS 12 or newer · Apple silicon and Intel | universal DMG | [![Download for macOS](https://img.shields.io/badge/Download_for_macOS-111827?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Windows 10 and 11 · x86_64 | `-setup.exe` or MSI | [![Download for Windows](https://img.shields.io/badge/Download_for_Windows-3A86FF?style=for-the-badge&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA0NDggNTEyIj48cGF0aCBmaWxsPSIjZmZmIiBkPSJtMCA5My43bDE4My42LTI1LjN2MTc3LjRIMHptMCAzMjQuNmwxODMuNiAyNS4zVjI2OC40SDB6bTIwMy44IDI4TDQ0OCA0ODBWMjY4LjRIMjAzLjh6bTAtMzgwLjZ2MTgwLjFINDQ4VjMyeiIvPjwvc3ZnPg==)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Linux · x86_64 | AppImage, DEB or RPM | [![Download for Linux](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest) |
| Linux · ARM64 | AppImage, DEB or RPM | [![Download for Linux](https://img.shields.io/badge/Download_for_Linux-FCC624?style=for-the-badge&logo=linux&logoColor=111827)](https://github.com/prajwal-svm/folderskin/releases/latest) |

The first release is still being prepared; until it's out, [build from source](#build-from-source).

The macOS app is signed and notarized by Apple, so it opens like any other. The Windows installer
isn't signed yet, so SmartScreen asks first: choose "More info", then "Run anyway". The Linux
packages need glibc 2.35 or newer (Ubuntu 22.04, Debian 12, Fedora 36 and later).

## Build from source

Prerequisites on every platform: [Rust](https://rustup.rs) (rustup installs the version
`rust-toolchain.toml` names on first use), Node 22 or newer, and pnpm 11 (`packageManager` in
`package.json` names the exact version).

- **macOS:** Xcode command line tools (`xcode-select --install`).
- **Windows:** Visual Studio Build Tools with the C++ workload, and the WebView2 runtime
  (already present on Windows 11).
- **Linux:**

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

Then:

```sh
pnpm install
pnpm tauri dev      # run it
pnpm tauri build    # installers land in target/release/bundle/
```

Checks, all of which CI runs ([docs/CI.md](docs/CI.md) lists every job):

```sh
cargo test --workspace
pnpm test
pnpm typecheck
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Contributing

Bug reports and skins are both welcome; skins go in through [community packs](docs/PACKS.md).
[CONTRIBUTING.md](CONTRIBUTING.md) covers the workflow and the few house rules;
[SECURITY.md](SECURITY.md) is how to report a vulnerability privately. Changes are recorded in
[CHANGELOG.md](CHANGELOG.md), and [docs/RELEASING.md](docs/RELEASING.md) is how a release is
built, signed and published.

If FolderSkin gave your folders a better look, a [GitHub star](https://github.com/prajwal-svm/folderskin)
helps other people find it.

## License

MIT — see [LICENSE](LICENSE). Copyright 2026 FolderSkin contributors. The bundled font is
Manrope under the SIL Open Font License (`assets/fonts/OFL.txt`); the built-in skins are CC0.
The animated icons are adapted from [lucide-animated](https://lucide-animated.com) (MIT) and
[Lucide](https://lucide.dev) (ISC); see
[src/components/icons/LICENSES.md](src/components/icons/LICENSES.md).

[Security](SECURITY.md) · [Contributing](CONTRIBUTING.md) · [MIT](LICENSE)

## Star history

<a href="https://www.star-history.com/#prajwal-svm/folderskin&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date" />
   <img alt="Star history chart" src="https://api.star-history.com/svg?repos=prajwal-svm/folderskin&type=Date" />
 </picture>
</a>
