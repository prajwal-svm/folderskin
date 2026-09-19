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
│   ├── build.rs             Tauri's build step; nothing else is embedded
│   ├── src/commands.rs      the library, import, apply and delete commands
│   ├── src/community.rs     community packs: list, preview, add, update, remove, share
│   ├── src/onboarding.rs    whether the first-launch onboarding has been finished
│   ├── src/pack_views.rs    packs looked through, kept drawn for a week
│   ├── src/ai.rs            the AI assistant's commands
│   ├── src/state.rs         what the commands share: the store and its caches
│   └── src/store.rs         saved skins on disk
├── crates/
│   ├── folderskin-core/     geometry · fit · raster · compositor · ico · matte · pack · apply
│   ├── folderskin-ai/       the AI providers: catalogue, requests, prompts
│   └── folderskin-tools/    CLI: make, check and index packs, render, guide, apply, revert
├── community/               the community packs, their index and their preview strips
├── assets/                  the app icon's source and the font
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
   [SKINS.md](SKINS.md) for what that crop keeps. A finished folder picture skips this step and
   the next ([below](#artwork-or-a-finished-folder)).
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
| `folderskin-core` | `geometry` (the template as vector paths), `fit` (cover-fit maths), `raster` (premultiplied downsampling, PNG encoding), `compositor` (the render), `ico` (Windows `.ico` writer with PNG entries), `matte` (keying and telling a finished folder from artwork), `pack` (the community pack contract and its checks), `apply` (per-OS icon writers) |
| `folderskin-ai` | the AI providers: the catalogue, each provider's request body and response reader, and the prompt templates |
| `folderskin-tools` | the maintainer CLI. Makes a community pack from pictures, checks the packs and writes their index, renders any picture as the folder the app makes of it, writes the safe-area guide, and applies or reverts an icon without the GUI — which is how the Windows and Linux writers get exercised |
| `folderskin` (`src-tauri`) | the Tauri app: window, commands, caches. Holds no drawing code |

`folderskin-core` has no Tauri dependency, so the CLI and the app share it and the core's
tests run without a webview.

## Frontend to backend

The commands live in `src-tauri/src/commands.rs` (the library), `community.rs` (packs),
`onboarding.rs` (first launch) and `ai.rs` (the AI assistant). Anything slow runs on a blocking
thread. `src/lib/tauri.ts` is the only place the frontend names them.

| command | input | output |
|---|---|---|
| `list_skins` | – | the user's skins, newest first, each with a PNG data-URL thumbnail; plus the plain default folder |
| `inspect_path` | `path` | `{kind: "folder" \| "image" \| "other", name, path}` |
| `import_image` | `path` | the picture saved as a skin, id `user:<hash>` (the saved one if it was imported before) |
| `apply_skin` | `folder`, `skinId` | `{}` or an error string |
| `revert_skin` | `folder` | `{}` or an error string |
| `delete_skin` | `skinId` | `{}`, or an error string for the plain default folder's id |
| `edit_skin` | `skinId`, `name`, `tags` | `{name, tags}` as saved (the name on one line, at most 60 characters; the tags cleaned, at most 8), or an error string for the plain default folder's id |
| `skins_folder` | – | the folder the saved skins live in |
| `community_packs` | `fresh` | the packs in `community/index.json` on GitHub, each with its `hash`, `added` and `update` |
| `community_preview` | `packId`, `fresh` | the pack's preview strip as a PNG data URL |
| `community_pack_skins` | `packId`, `hash` | every skin of the pack drawn as its folder, to look through; kept drawn for a week, so looking again at that `hash` downloads nothing |
| `community_add` | `packId`, `onProgress` | the pack's skins in the pack's order, saved all together or not at all; progress on the channel (below) |
| `community_update` | `packId` | `{removed, skins}`: the added pack swapped for the version on GitHub now |
| `community_remove` | `packId` | the ids of the skins it deleted |
| `import_pack` | `path` | a pack folder on disk, added the same way as one from GitHub |
| `export_pack` | `folder`, `name`, `author`, `license`, `tags`, `skinIds` | the pack folder it wrote, already passing the checks |
| `onboarding_needed` | – | `true` until the first-launch onboarding has been finished on this computer |
| `finish_onboarding` | – | `{}`; the onboarding never shows again |
| `folder_icon` | `folder` | the folder's current icon as a PNG data URL (the real one from the OS on macOS) |
| `platform_info` | – | `{os, browse_label, note}` |

`ai_generate` returns the same skin shape as `import_image`. A skin is:

```ts
{ id, name, thumbnail,
  collection, custom,                   // always "yours" and true: every skin is the user's own
  kind: "artwork" | "folder",           // wrapped onto our template, or a finished folder image
  source: "import" | "ai" | "community",
  created_at: number,                   // Unix ms when it was saved
  tags: string[],                       // the library's filters
  pack, pack_name, author, license,     // community skins: where it came from, for credit
  made_with, idea }                     // AI results: "OpenAI · GPT Image 2.5 Flare", the prompt
```

FolderSkin ships no skins of its own, so every skin in the list is a saved one (or one kept for
this session when it could not be written). The pack contract is `folderskin_core::pack`, shared
by the app and `folderskin-tools packs check`; see [PACKS.md](PACKS.md).

`community_add` takes a `Channel<PackProgress>` from `@tauri-apps/api/core` as `onProgress` and
sends `{stage: "download" | "save", done, total}` on it, `total` being the pack's number of
pictures:

1. `{stage: "download", done: 0}` once `pack.json` is in, then `done` counting up as each
   picture arrives, to `total`.
2. `{stage: "save", done: 0}` once every picture is in, then `done` counting up as each new
   picture is encoded and ready to write (pictures already in the library are not counted), and
   last `{stage: "save", done: total}` once everything is saved.

The command's result, after the last message, is what says the pack was added. Nothing is sent
when it fails before the pack's list arrives, and a message the window can no longer receive is
dropped without failing the command.

Errors cross the boundary as plain strings already written for a person ("couldn't read that
picture", or the reason the OS gave), because the drop zone shows them verbatim. There is no
error-code vocabulary to translate.

Drag and drop uses Tauri's native `onDragDropEvent` rather than HTML5 drops, because an HTML5
drop in a webview cannot expose a filesystem path. Browsing uses the dialog plugin.

## State and caching

`AppState` (in `src-tauri/src/state.rs`) is an `Arc` over:

- `store: OnceLock<Store>` — the saved skins on disk (below), opened in `setup` before the
  window exists.
- `recent` — the twelve most recently used saved skins, decoded, least recently used out first.
  It is only a cache: `apply_skin` reads a saved skin back from the store on a miss, so
  evicting one loses nothing. A single import or AI result goes in as it is saved; a pack does
  not (`AppState::save_many`), since sixteen new skins would push out every other one.
- `unsaved` — skins that could not be written (no app data folder, or a failed write of an AI
  result, which is never thrown away). They last until the app quits.
- `default_thumb: OnceLock<String>` — the plain default folder's thumbnail as a data URL, drawn
  once. It is also cached as `thumbs/default.thumb-v2.png` in the app cache directory, so later
  launches skip the render.

Packs looked through in Community are kept drawn in `pack-views/` in the app cache directory
(`src-tauri/src/pack_views.rs`), one file per pack version, named after its hash. A file older
than a week is deleted when it is next read or when another pack is kept, and so is a pack's
old version once its new one is kept.

What persists: the saved skins, that thumbnail, the packs looked through, the onboarding marker
(below), the AI keys ([AI.md](AI.md)) and the favourites list in the webview's `localStorage`.
There is no database, and no network access outside the AI assistant, the community packs and
the update check.

## Saved skins

Every picture the user imports, every AI result and every skin of a community pack is saved as
soon as it arrives, by `src-tauri/src/store.rs`, in a `skins` folder in the app data directory:

| | |
|---|---|
| macOS | `~/Library/Application Support/app.folderskin.desktop/skins/` |
| Windows | `%APPDATA%\app.folderskin.desktop\skins\` |
| Linux | `~/.local/share/app.folderskin.desktop/skins/` (or under `$XDG_DATA_HOME`) |

```
skins/
├── skins.json                 index: {"version": 1, "skins": [...]}
├── 3f2a9c0b1d4e.png           the skin, longest side at most 2048 px
└── 3f2a9c0b1d4e.thumb-v2.png  its 512 px gallery thumbnail
```

Each index entry records the id, name, tags, kind (`artwork` or `folder`), source (`import`,
`ai` or `community`), `created_at` in Unix milliseconds, the focus point for artwork, for AI
results the provider, model and the user's own words, and for community skins the pack's id,
name, author, licence and hash. The id is `user:` plus the first 12 hex digits of the
SHA-256 of what came in (the picture file, or the provider's image), so importing the same
picture twice finds the skin already saved. Ids from the webview are checked against that exact
shape before they name a file.

Pictures and thumbnails are written before the index entry that names them, and every file is
written atomically (temp file, then rename), so a crash leaves at worst an unreferenced picture.
An index that cannot be read is renamed `skins-unreadable-<ms>.json` and the store starts
empty; a damaged entry, or one whose picture has gone, is dropped with a log line. The
thumbnail file name carries `THUMB_CACHE_VERSION`, so bumping it redraws saved thumbnails too.

A pack is saved in one go by `Store::add_many`: every new picture and thumbnail is encoded first,
on all cores and outside the index lock, then all of their files are written, then the index,
once. If any of it fails, the files that call wrote are removed and the index stays exactly as
it was, so a pack is never half added. A picture already saved, or listed twice, comes back as
it was saved. The pack's first skin gets the newest `created_at` and each after it an older one,
all newer than anything saved before, so the library, newest first, shows a pack in its own
order.

`Store::open` then clears away what a crash left: pictures (`<stem>.png`) and thumbnails
(`<stem>.thumb*.png`) of skins the index doesn't name, and `write_atomic` temp files
(`.<name>.folderskin-<pid>-<seq>.tmp`), each logged. It only does so when the index was read
whole (no entry dropped as damaged or with a bad id; a missing picture or a repeated entry is
fine) or there is no index at all, and no `skins-unreadable-*.json` was ever set aside there,
since otherwise a file the index doesn't name may belong to a skin it lost. It never touches a
file of any other name, and leaves anything changed in the last ten minutes, which another
FolderSkin could still be saving.

### First launch

The onboarding shows until it is finished once. `finish_onboarding` writes `onboarding.json`
beside the `skins` folder, in the app data directory:

```
{"version":1,"finished_at":1790000000000,"app":"0.1.0"}
```

`onboarding_needed` is `true` while that file is missing, `false` when the app has no data
folder (nothing could remember it being finished), and always `true` when the environment has
`FOLDERSKIN_ONBOARDING=1`, for trying the onboarding out. Only whether the file exists decides
anything; what it records is there for a later version.

### Artwork or a finished folder

An imported picture becomes one of two kinds (`matte::surround` and `matte::finished_cutout` in
the core):

- **folder, with real transparency**: at least half of an outer band (1% of the shorter side)
  has alpha at or below 16, or all four corners do and at least a fifth of the band does (a
  cut-out trimmed tight to its outline). The picture is trimmed to its visible pixels.
- **folder, on the magenta key**: `has_key_background` holds at a strict tolerance of 0.12
  (60% of the outermost ring within 0.12 of #FF00FF), or all four corners and a fifth of the
  band are on the key. The keyer's own tolerance, 0.18, is too loose to decide this: a product
  shot on magenta paper or a vivid sunset sky sits around 0.14 to 0.17 and must stay a picture.
  The backdrop is keyed out, despilled and trimmed like an AI render; if less than 2% of the
  picture is left, it was not a folder and becomes artwork.
- **artwork** otherwise, with the focus in the middle.

A finished folder image is used as the icon as it is, through `compositor::icon_set_from_image`,
and is never wrapped in the template a second time.

## Community packs

FolderSkin ships no skins: `src-tauri/build.rs` is only Tauri's own build step, and every skin
comes from the user's store. The folder template and the plain default folder are code in
`folderskin-core`, not pictures.

`community.rs` reads the packs straight from the repository on GitHub (`FOLDERSKIN_COMMUNITY_URL`
points it at another copy). Adding one downloads its `pack.json`, then its pictures four at a
time (`futures_util`'s `buffered`, which keeps them in the pack's order), each held to the
limits in `folderskin_core::pack` and hashed with the manifest in the pack's order
(`pack::pack_hash`), which is how an update is noticed later. The pictures are then decoded and
told apart, finished folder or artwork, on all cores (`prepare_import`), and only when every one
has passed are they saved together (`AppState::save_many`). `community_pack_skins` and
`community_update` download the same way without reporting progress, and `import_pack` reads a
pack folder from disk.

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
| `pack` | the pack contract: fields, limits, tags, ids, file names, picture checks and the pack hash |
| `matte` | keying, despill and trim; telling a finished folder (transparent, keyed, keyed then JPEG-compressed, trimmed tight) from an ordinary photo, a pink sunset and a product shot on magenta paper |
| `store` | round trip across a restart, one entry per picture, delete, a corrupt or missing index, damaged entries, thumbnail repair, the size bound, id checks; a batch saved in its own order, whole or not at all (a write that fails part way, an index that can't be written), saved and repeated pictures; what a crash left removed on open, and nothing else |
| `community` | a pack saved in its own order with its progress, a picture listed twice, a pack that can't be saved adding nothing, updates, and the download: four at a time, in order, with its progress, against a local server |
| `onboarding` | when it shows, what forces it, and the marker |
| `folderskin-tools` | the pack checks and the index, making a pack (the split, `--flat-backdrop`, WebP), the picture split `render` and `apply` use |
| frontend | the drop-zone reducer, favourites, platform copy (vitest) |

The Windows writer is compile-checked from macOS with `cargo check --target
x86_64-pc-windows-msvc -p folderskin-core`. CI runs the whole set on ubuntu-22.04,
windows-latest and macos-latest.

## The AI assistant

`crates/folderskin-ai` talks to the AI providers, and only when the user presses Generate. It holds the provider catalogue, the per-provider request bodies and response
readers (pure functions, unit-tested without a network) and the prompt templates. The app crate
keeps the keys, encrypted in a private file rather than in the keychain (`src-tauri/src/keys.rs`
explains how and why). `crates/folderskin-core/src/matte.rs` turns a keyed render into a clean cutout for the
providers that cannot return an alpha channel. [AI.md](AI.md) covers the feature itself.

A finished folder image, generated whole or imported, is applied without going through the
compositor: `compositor::icon_set_from_image` fits it into the icon canvas instead. That is the
one path where the icon's geometry is not ours, which is why a whole-folder generation sends
the model our own blank template to repaint whenever the model accepts a picture
(`compositor::blank_template`).

## Updates

`tauri-plugin-updater` does the work. It reads the newest public release's `latest.json`
(`plugins.updater` in `tauri.conf.json`), downloads this platform's installer and checks its
minisign signature against the `pubkey` built into the app before it installs anything.
`tauri-plugin-process` then restarts into the new version. The page decides when to ask:

- `src/lib/updater.ts` wraps the plugin.
- `src/hooks/useUpdates.ts` asks a few seconds after a release build opens (never in
  development), and whenever About or Settings asks.
- `src/components/UpdateDialog.tsx` shows the notes and the progress. It can't be closed while
  an update installs.

The plugin's own TLS feature is off. It uses the app's reqwest, with rustls on aws-lc-rs, so
`ring` isn't compiled in. The signing key and the feed that `.github/workflows/release.yml`
assembles are covered in [RELEASING.md](RELEASING.md#updates).

A release build is locked down as an app (`src/lib/lockdown.ts`): no right-click menu, and no
reload or inspector shortcuts. `tauri` has no `devtools` feature, so release builds have no web
inspector at all.

## Size budget

Under 15 MB installed. The macOS app bundle (Apple Silicon) is the stripped release binary, the
1.4 MB app icon and a 1 KB `Info.plist`. The binary carries the Rust code and the frontend,
which Tauri embeds, including a 165 KB variable font (Manrope) and the first-launch welcome's
732 KB of pictures (26 folders, each drawn at the size it's shown at, and the logo). Once FolderSkin stopped shipping skins the binary measured 7.0 MB and the
bundle 8.5 MB; with 2.3 MB of built-in skins they had been 8.7 MB and 10.2 MB. The updater
added 0.25 MB, to a 7.3 MB binary and an 8.8 MB bundle. `image` is built with `default-features = false` and
only `png`, `jpeg` and `webp`, and the release profile uses `opt-level = "s"`, LTO and one
codegen unit. Any dependency that would move this budget needs a reason in the pull request.

Vite copies everything in `public/` into every build, and Tauri embeds the build in the binary,
so dev-only files stay out of `public/`. The browser mock (`src/lib/devMock.ts`) shows the
pictures in `community/packs/`, which Vite serves from the repository in dev only; skin previews
that once sat in `public/` added 2.3 MB to the binary without showing up as files in the bundle.
