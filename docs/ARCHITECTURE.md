# Architecture

FolderSkin is a Tauri v2 app: a Rust core that renders and writes folder icons, and a React
webview that is only a user interface. The design decision everything else follows from is
that **there is exactly one rendering path**.

The geometry constants that define the folder template live in
`crates/folderskin-core/src/geometry.rs`, documented in place and covered by silhouette tests.

## Layout

```
folderskin/
├── src/                     React 19 + TypeScript webview
│   ├── state/               drop-zone reducer, favourites (unit-tested)
│   ├── lib/                 typed invoke wrappers, platform strings
│   ├── hooks/               native drag-and-drop
│   └── components/          TabBar, Gallery, FolderThumb, DropZone, Wordmark, AboutMenu
├── src-tauri/               the app crate: commands, state, window config
│   ├── build.rs             embeds assets/skins into the binary
│   └── src/commands.rs      the six commands the frontend can call
├── crates/
│   ├── folderskin-core/     geometry · fit · raster · compositor · ico · manifest · apply
│   └── folderskin-tools/    CLI: generate and import skins, render, check, apply, revert
├── assets/skins/            manifest.json + the ten shipped skins
└── docs/
```

## One rendering path

Every pixel the user ever sees — a gallery thumbnail, the drop-zone preview, the icon
written to disk — comes out of `folderskin_core::compositor::render_icon_set`. The webview
never draws folder geometry; it displays PNGs the Rust side rendered and handed over as data
URLs. A thumbnail is therefore a correct preview of the icon by construction, on every
operating system, and there is no second implementation to keep in sync.

The compositor:

1. Cover-fits the artwork into the back panel's bounding box and the front panel's rectangle
   (`fit::cover_fit`, `geometry`), cropping around the skin's focus point. See
   [SKINS.md](SKINS.md) for what that crop keeps.
2. Draws the template with `tiny-skia`: back panel with the tab, the cream paper sheet, the
   front panel, a white rim light on the top and sides, a shade along the bottom. No drop
   shadow.
3. Renders once at 2048 px and downsamples to 1024, 512, 256, 128, 64, 48, 32, 24 and 16 with
   Lanczos3 on premultiplied alpha. Downsampling from one master is what gives the soft
   two-pixel edge the design asks for, and it keeps the small sizes from turning to mush.

Geometry lives in one place, as constants with unit tests that assert every part stays inside
the canvas and that the template is left–right symmetric. Nothing computes coordinates from
a rendered bitmap.

## Crates

| crate | responsibility |
|---|---|
| `folderskin-core` | `geometry` (the template as vector paths), `fit` (cover-fit maths), `raster` (premultiplied downsampling, PNG encoding), `compositor` (the render), `ico` (Windows `.ico` writer with PNG entries), `manifest` (the skin list and its validation), `apply` (per-OS icon writers) |
| `folderskin-tools` | the maintainer CLI. Generates the ten built-in skins deterministically, imports a picture as a skin, renders previews, validates the manifest, and applies or reverts an icon without the GUI — which is how the Windows and Linux writers get exercised |
| `folderskin` (`src-tauri`) | the Tauri app: window, commands, caches. Holds no drawing code |

`folderskin-core` has no Tauri dependency, so the CLI and the app share it and the core's
tests run without a webview.

## Frontend to backend

Seven commands, all in `src-tauri/src/commands.rs`, all async with the heavy work on a blocking
thread. `src/lib/tauri.ts` is the only place the frontend names them.

| command | input | output |
|---|---|---|
| `list_skins` | – | the ten skins with PNG data-URL thumbnails, plus the plain default folder |
| `inspect_path` | `path` | `{kind: "folder" \| "image" \| "other", name, path}` |
| `import_image` | `path` | a skin with id `custom:<hash>` and a thumbnail |
| `apply_skin` | `folder`, `skinId` | `{}` or an error string |
| `revert_skin` | `folder` | `{}` or an error string |
| `folder_icon` | `folder` | the folder's current icon as a PNG data URL (the real one from the OS on macOS) |
| `platform_info` | – | `{os, browse_label, note}` |

Errors cross the boundary as plain strings already written for a person ("couldn't read that
picture", or the reason the OS gave), because the drop zone shows them verbatim. There is no
error-code vocabulary to translate.

Drag and drop uses Tauri's native `onDragDropEvent` rather than HTML5 drops, because an HTML5
drop in a webview cannot expose a filesystem path. Browsing uses the dialog plugin.

## State and caching

`AppState` (in `src-tauri/src/state.rs`) is an `Arc` over three things:

- `builtin: OnceLock<Vec<LoadedSkin>>` — the embedded skins decoded to RGBA on first use and
  kept for the process lifetime, so applying a skin never re-decodes a JPEG.
- `custom: Mutex<HashMap<String, Arc<Artwork>>>` — pictures the user dropped this session,
  keyed `custom:<sha256[..12]>`, downscaled so the longer side is at most 2048 px.
- `thumbs: Mutex<HashMap<String, String>>` — rendered thumbnails as data URLs. Built-in skins
  and the default folder are also written to the app cache directory so later launches skip the
  render; imported pictures stay in memory, and the map holds the twelve most recent.

Nothing is persisted apart from that cache and the favourites list, which lives in the
webview's `localStorage`. There is no database, no config file and no network access.

## Skins are embedded at build time

`src-tauri/build.rs` reads `assets/skins/manifest.json`, emits a `SKINS` table into
`$OUT_DIR/skins_gen.rs` with one `include_bytes!` per skin, and `src-tauri/src/skins.rs`
`include!`s it. The app therefore ships as a single binary with no asset directory to lose,
and a missing or malformed manifest fails the build rather than the app.

The consequence for authors: changing a skin requires a rebuild. See [SKINS.md](SKINS.md).

## Applying an icon

`apply_skin` resolves the artwork, renders the icon set, validates the folder path, then calls
the platform writer. On macOS the `NSWorkspace` call is marshalled to the main thread and
awaited. Every writer refuses anything that is not an existing directory, refuses filesystem
roots, and writes atomically. What each platform actually writes, and what revert undoes, is
in [PLATFORMS.md](PLATFORMS.md).

## Frontend state

The drop zone is a reducer in `src/state/dropzone.ts` with six phases — `idle`, `folder`,
`ready`, `applying`, `applied`, `reverting` — and it is unit-tested in isolation from React.
An error is a field on the state, not a phase, so a failed apply returns to `ready` with the
message shown beneath the button.
Components read the state and render; they do not decide transitions. Dropping a picture
selects a custom skin without changing which folder is chosen, which is why picking a folder
and picking a skin are separate axes in the machine.

Per the project style rule there are no CSS outlines, focus rings or selection outlines
anywhere; focus and selection are shown with a background tint or a border colour change.

## Testing

| layer | what is tested |
|---|---|
| `geometry` | every part inside the canvas, symmetry, path closure |
| `fit` | cover-fit always covers, focus extremes keep the expected corner |
| `compositor` | determinism (same input, identical bytes) and silhouette checks against the measured constants at sample rows and columns |
| `ico` | round-trip of the multi-size container |
| `apply` | `desktop.ini` and `.directory` generation and revert parsing as pure functions; path validation |
| `manifest` | parsing with defaults, and validation of ids, sizes, dimensions and byte budgets |
| frontend | the drop-zone reducer, favourites, platform copy (vitest) |

The Windows writer is compile-checked from macOS with `cargo check --target
x86_64-pc-windows-msvc -p folderskin-core`. CI runs the whole set on ubuntu-22.04,
windows-latest and macos-latest.

## The AI assistant

`crates/folderskin-ai` is the only crate that talks to the network, and only when the user
presses Generate. It holds the provider catalogue, the per-provider request bodies and response
readers (pure functions, unit-tested without a network), the prompt templates, and the keychain
wrapper. `crates/folderskin-core/src/matte.rs` turns a keyed render into a clean cutout for the
providers that cannot return an alpha channel. [AI.md](AI.md) covers the feature itself.

A generated whole-folder image is applied without going through the compositor:
`compositor::icon_set_from_image` fits it into the icon canvas instead. That is the one path
where the icon's geometry is not ours.

## Size budget

Under 15 MB installed. As of 0.1.0 the macOS app bundle (Apple Silicon) is 10.2 MB and its
DMG is 8.5 MB. The bundle is an 8.7 MB stripped release binary, the 1.4 MB app icon and a
1 KB `Info.plist`. The binary carries the Rust code and everything embedded at build time: the
2.3 MB of built-in skins (`src-tauri/build.rs`) and the frontend, 540 KB before compression
including a 165 KB variable font (Manrope). `image` is built with `default-features = false`
and only `png`, `jpeg` and `webp`, and the release profile uses `opt-level = "s"`, LTO and one
codegen unit. Any dependency that would move this budget needs a reason in the pull request.

Vite copies everything in `public/` into every build, and Tauri embeds the build in the binary,
so dev-only files stay out of `public/`. The browser mock reads its skin previews from
`assets/previews`, which Vite serves in dev only; while they sat in `public/` they added 2.3 MB
to the binary without showing up as files in the bundle.
