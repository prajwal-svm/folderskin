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
│   ├── composer/            the composer's design document, undo, drawing and maths (unit-tested)
│   └── components/          TabBar, Gallery, FolderThumb, DropZone, Wordmark, AboutMenu, composer/
├── src-tauri/               the app crate: commands, state, window config
│   ├── build.rs             Tauri's build step. Nothing else is embedded
│   ├── src/commands.rs      the library, import, apply and delete commands
│   ├── src/community.rs     community packs: list, preview, add, update, remove, save as a folder
│   ├── src/share.rs         sharing a pack through the community service, and trying again
│   ├── src/deep_link.rs     folderskin://install links from the website's Install buttons
│   ├── src/installs.rs      telling the community service a pack was added, for its count
│   ├── src/onboarding.rs    whether the first-launch onboarding has been finished
│   ├── src/pack_views.rs    packs looked through, kept drawn for a week
│   ├── src/ai.rs            the AI assistant's commands
│   ├── src/composer.rs      the composer's commands: the template's layers, saving a design
│   ├── src/state.rs         what the commands share: the store and its caches
│   └── src/store.rs         saved skins on disk
├── crates/
│   ├── folderskin-core/     geometry · fit · raster · compositor · ico · matte · pack · apply
│   ├── folderskin-ai/       the AI providers: catalogue, requests, prompts
│   └── folderskin-tools/    CLI: make, check and index packs, render, guide, apply, revert
├── assets/                  the app icon's source and the font
└── docs/
```

## One rendering path

Every pixel the user ever sees comes out of `folderskin_core::compositor::render_icon_set`: a
gallery thumbnail, the drop-zone preview, the icon written to disk. The webview never draws
folder geometry. It displays PNGs the Rust side rendered and handed over as data URLs. A
thumbnail is therefore a correct preview of the icon by construction, on every operating system,
and there is no second implementation to keep in sync.

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

The composer ([COMPOSER.md](COMPOSER.md)) is the one place the webview draws on a canvas, and it
draws only the user's own art: text, emoji, shapes, pictures and patterns. The folder around the
art is still the compositor's. `compositor::template_layers` splits the template into the layers
a design sits between (the back and front panels' coverage, the paper and back rim between them,
the front's rims on top), and the stage stacks the design between them. The saved icon comes
from `compositor::render_master_placed`, which fills both panels with the design where it was
drawn. The two share their drawing code, and a test holds the stacked layers within 3 levels of
the saved icon, so the canvas shows what gets written.

## Crates

| crate | responsibility |
|---|---|
| `folderskin-core` | `geometry` (the template as vector paths), `fit` (cover-fit maths), `raster` (premultiplied downsampling, PNG encoding, and WebP through libwebp), `compositor` (the render, for artwork, a finished folder or a design drawn in place, and the template split into the composer's layers), `ico` (Windows `.ico` writer with PNG entries), `matte` (keying and telling a finished folder from artwork), `pack` (the community pack contract and its checks), `apply` (per-OS icon writers) |
| `folderskin-ai` | the AI providers: the catalogue, each provider's request body and response reader, and the prompt templates |
| `folderskin-tools` | the maintainer CLI. Makes a community pack from pictures, checks the packs and writes their index, renders any picture as the folder the app makes of it, writes the safe-area guide, and applies or reverts an icon without the GUI, which is how the Windows and Linux writers get exercised |
| `folderskin` (`src-tauri`) | the Tauri app: window, commands, caches. Holds no drawing code |

`folderskin-core` has no Tauri dependency, so the CLI and the app share it and the core's
tests run without a webview.

## Frontend to backend

The commands live in `src-tauri/src/commands.rs` (the library), `community.rs` (packs),
`deep_link.rs` (install links), `onboarding.rs` (first launch), `ai.rs` (the AI assistant), `composer.rs` (the composer, whose
six commands are listed in [COMPOSER.md](COMPOSER.md#commands)) and `tree.rs` (a folder and its
subfolders). Anything slow runs on a blocking
thread. `src/lib/tauri.ts` is the only place the frontend names them.

| command | input | output |
|---|---|---|
| `list_skins` | – | the user's skins, newest first, each with a PNG data-URL thumbnail, and beside them the plain default folder |
| `inspect_path` | `path` | `{kind: "folder" \| "image" \| "other", name, path}` |
| `import_image` | `path` | the picture saved as a skin, id `user:<hash>` (the saved one if it was imported before) |
| `apply_skin` | `folder`, `skinId` | `{}` or an error string |
| `revert_skin` | `folder` | `{}` or an error string |
| `subfolder_count` | `folder` | `{folder, count, done}`: starts counting the folders inside it in the background (the count before it stops), and answers once it's done or after a moment. The rest comes as `subfolder-count` events |
| `subfolder_counts` | `folder`, `paths` | `{folder, count, done, paths}`: how far that count has got, and for each of `paths` (folders in it) `{found, inside, done}` |
| `subfolder_list` | `folder` | `{path, separator, names, nested}`: one folder's own folders, for a column of **Choose subfolders**, and whether each has folders inside (`null` when there wasn't time to look) |
| `tree_bytes` | `skinId` | the bytes of disk one folder's copy of the skin's icon takes |
| `start_tree_apply` | `folder`, `skinId`, `choice` | `{seq, run}`: starts putting the skin on the folder and the folders inside it `choice` takes (all of them without one) in the background, refused while another run is going. The rest comes as `tree-run` events |
| `start_tree_revert` | `folder`, `choice`, `skipPlain` | the same, taking the custom icons off. `skipPlain` leaves the folders without one alone |
| `stop_tree_run` | `id` | `{}`, and run `id` stops before its next folder |
| `carry_on_tree_run`, `retry_tree_run`, `undo_tree_run` | `id` | `{seq, run}`: run `id` carries on with the folders it didn't reach, tries the ones it couldn't change again, or has exactly the folders it changed taken off, as a run of its own |
| `dismiss_tree_run` | `id` | `{}`, and run `id` is forgotten once it has ended |
| `tree_run` | – | `{seq, run}`: the latest run, going or ended, or `run: null` |
| `delete_skin` | `skinId` | `{}`, or an error string for the plain default folder's id |
| `edit_skin` | `skinId`, `name`, `tags` | `{name, tags}` as saved (the name on one line and at most 60 characters, the tags cleaned and at most 8), or an error string for the plain default folder's id |
| `skins_folder` | – | the folder the saved skins live in |
| `community_packs` | `fresh` | `{packs, moved}`: the featured packs, or the first few (each with its `hash`, `added`, `update` and `official`), and each old id that moved to one of them |
| `community_pack` | `packId` | that pack, or the one an old id moved to, as the list shows it, or `null`. One the list doesn't have is looked for again past the caches first |
| `install_link_take` | – | the pack the newest `folderskin://install` link asked for, once, or `null` when none is waiting |
| `community_preview` | `packId`, `fresh` | the pack's preview strip as a PNG data URL |
| `community_pack_skins` | `packId`, `hash` | every skin of the pack drawn as its folder, to look through, and kept drawn for a week, so looking again at that `hash` downloads nothing |
| `community_add` | `packId`, `onProgress` | the pack's skins in the pack's order, saved all together or not at all, with progress on the channel (below) |
| `community_update` | `packId` | `{removed, skins}`: the added pack swapped for the version published now |
| `community_remove` | `packId` | the ids of the skins it deleted |
| `import_pack` | `path` | a pack folder on disk, added the same way as one from Community |
| `export_pack` | `folder`, `name`, `author`, `license`, `tags`, `skinIds` | the pack folder it wrote, named after a new id (`pack::new_id`), already passing the checks |
| `onboarding_needed` | – | `true` until the first-launch onboarding has been finished on this computer |
| `finish_onboarding` | – | `{}`, and the onboarding never shows again |
| `folder_icon` | `folder` | the folder's current icon as a PNG data URL (the real one from the OS on macOS) |
| `platform_info` | – | `{os, browse_label, note}` |

`ai_generate` returns the same skin shape as `import_image`. A skin is:

```ts
{ id, name, thumbnail,
  collection, custom,                   // always "yours" and true: every skin is the user's own
  kind: "artwork" | "folder",           // wrapped onto our template, or a finished folder image
  source: "import" | "ai" | "community" | "composer",
  created_at: number,                   // Unix ms when it was saved
  tags: string[],                       // the library's filters
  pack, pack_name, author, license,     // community skins: where it came from, for credit
  made_with, idea }                     // AI results: "OpenAI · GPT Image 2.5 Flare", the prompt
```

FolderSkin ships no skins of its own, so every skin in the list is a saved one (or one kept for
this session when it could not be written). The pack contract is `folderskin_core::pack`, shared
by the app and `folderskin-tools packs check`. See [PACKS.md](PACKS.md).

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

- `store: OnceLock<Store>`: the saved skins on disk (below), opened in `setup` before the
  window exists.
- `recent`: the twelve most recently used saved skins, decoded, least recently used out first.
  It is only a cache: `apply_skin` reads a saved skin back from the store on a miss, so
  evicting one loses nothing. A single import or AI result goes in as it is saved, but a pack
  does not (`AppState::save_many`), since sixteen new skins would push out every other one.
- `unsaved`: skins that could not be written (no app data folder, or a failed write of an AI
  result, which is never thrown away). They last until the app quits.
- `default_thumb: OnceLock<String>`: the plain default folder's thumbnail as a data URL, drawn
  once. It is also cached as `thumbs/default.thumb-v2.png` in the app cache directory, so later
  launches skip the render.

Packs looked through in Community are kept drawn in `pack-views/` in the app cache directory
(`src-tauri/src/pack_views.rs`), one file per pack version, named after its hash. A file older
than a week is deleted when it is next read or when another pack is kept, and so is a pack's
old version once its new one is kept.

What persists: the saved skins, that thumbnail, the packs looked through, the onboarding marker
(below), the AI keys ([AI.md](AI.md)) and the favourites list in the webview's `localStorage`.
There is no database, and no network access outside the AI assistant, the community packs, the
install count sent when a pack from Community is added ([below](#install-links-and-counts)) and
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
├── 3f2a9c0b1d4e.webp          the skin, a lossless WebP, longest side at most 2048 px
├── 3f2a9c0b1d4e.thumb-v2.png  its 512 px gallery thumbnail
└── 7c01e5a9b2d8.design.json   a design's document, beside a skin made in the composer
```

A new picture is saved as a lossless WebP at libwebp's quickest setting, which takes about the
time a PNG did and is about a third smaller. Pictures saved by 0.1.6 and before are PNGs
(`<stem>.png`). They are read as they are and never written again, and go with their skin when
it's deleted.

Each index entry records the id, name, tags, kind (`artwork` or `folder`), source (`import`,
`ai`, `community` or `composer`), `created_at` in Unix milliseconds, the focus point for artwork, for AI
results the provider, model and the user's own words, and for community skins the pack's id,
name, author, licence and hash. The id is `user:` plus the first 12 hex digits of the
SHA-256 of what came in (the picture file, or the provider's image), so importing the same
picture twice finds the skin already saved. Ids from the webview are checked against that exact
shape before they name a file.

A design from the composer is saved as a finished folder, with its document beside it
(`<stem>.design.json`) so it can be opened again. The document is written first, it goes with the
skin when the skin is deleted, and one left behind by a crash is cleared away like a picture.
Saving a changed design over the old one (`Store::replace_design`) writes the new files, then the
index once with the old entry swapped for the new one at the old one's `created_at`, then removes
the old files.

Pictures and thumbnails are written before the index entry that names them, and every file is
written atomically (temp file, then rename), so a crash leaves at worst an unreferenced picture.
An index that cannot be read is renamed `skins-unreadable-<ms>.json` and the store starts
empty. A damaged entry, or one whose picture has gone, is dropped with a log line. The
thumbnail file name carries `THUMB_CACHE_VERSION`, so bumping it redraws saved thumbnails too.

A pack is saved in one go by `Store::add_many`: every new picture and thumbnail is encoded first,
on all cores and outside the index lock, then all of their files are written, then the index,
once. If any of it fails, the files that call wrote are removed and the index stays exactly as
it was, so a pack is never half added. A picture already saved, or listed twice, comes back as
it was saved. The pack's first skin gets the newest `created_at` and each after it an older one,
all newer than anything saved before, so the library, newest first, shows a pack in its own
order.

`Store::open` then clears away what a crash left: pictures (`<stem>.webp`, or `<stem>.png`) and thumbnails
(`<stem>.thumb*.png`) of skins the index doesn't name, and `write_atomic` temp files
(`.<name>.folderskin-<pid>-<seq>.tmp`), each logged. It only does so when the index was read
whole (no entry dropped as damaged or with a bad id, though a missing picture or a repeated entry
is fine) or there is no index at all, and no `skins-unreadable-*.json` was ever set aside there,
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
anything. What it records is there for a later version.

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
  The backdrop is keyed out, despilled and trimmed like an AI render. If less than 2% of the
  picture is left, it was not a folder and becomes artwork.
- **artwork** otherwise, with the focus in the middle.

A finished folder image is used as the icon as it is, through `compositor::icon_set_from_image`,
and is never wrapped in the template a second time.

## Community packs

FolderSkin ships no skins: `src-tauri/build.rs` is only Tauri's own build step, and every skin
comes from the user's store. The folder template and the plain default folder are code in
`folderskin-core`, not pictures.

`community.rs` reads the packs from packs.folderskin.app, where the published tree of their
repository, [folderskin-community](https://github.com/prajwal-svm/folderskin-community), is copied
as it's published, and from the repository on GitHub when that doesn't answer with a head.json
(`catalog::Origin`). `FOLDERSKIN_COMMUNITY_URL` points it at another copy of the repository alone.
Every file is checked against what head.json and the catalog say it is, wherever it came from.
Adding one downloads its `pack.json`, then its pictures four at a
time (`futures_util`'s `buffered`, which keeps them in the pack's order), each held to the
limits in `folderskin_core::pack` and hashed with the manifest in the pack's order
(`pack::pack_hash`), which is how an update is noticed later. The pictures are then decoded and
told apart, finished folder or artwork, on all cores (`prepare_import`), and only when every one
has passed are they saved together (`AppState::save_many`). `community_pack_skins` and
`community_update` download the same way without reporting progress, and `import_pack` reads a
pack folder from disk.

A pack is official when folderskin-community's `official.json` lists it: `head.json` carries the
list and `index.json` marks each such entry, and every pack the commands hand the webview says
`official`, which Community and the pack viewer show as a badge ([PACKS.md](PACKS.md#featured-and-official-packs)).

A pack's id can change: `moved.json` beside `packs/` maps each old id to the new one, and
`head.json` carries it as `moved`. Every id that comes from outside the catalog (an install link,
the webview, the library) is looked up through `Source::current_id`, and the first command to use
a catalog with moves moves the library's records of packs added under an old id to the new one
(`community::follow_moves`, `Store::move_packs`), so those packs stay added and keep getting
their updates.

### Install links and counts

`folderskin://install?pack=<id>` is the scheme the Install buttons on folderskin.app open
(`src-tauri/src/deep_link.rs`, with the steps in [PACKS.md](PACKS.md#the-install-link)).
`tauri-plugin-deep-link` delivers the link: on macOS as an event to the running app, on Windows
and Linux as the argument of a second process, which `tauri-plugin-single-instance` (registered
first, on those two only) hands to the running one before it quits. `deep_link::install_pack` is
the one function that reads a link, and anything that isn't an install link naming a pack id is
dropped. A good one brings the window forward and waits in `InstallLinks` until the webview takes
it with `install_link_take`. `install-link` tells the webview one is waiting. The webview asks as
it starts too (`src/lib/installLinks.ts`), so a link that came before it, or during the
first-launch welcome, isn't lost. The Community store's `install` then opens the pack from
`community_pack` and adds it through the same `add` its Add button uses.

Once `community_add` has saved a pack, `installs::report` sends
`POST <service>/v1/packs/<id>/installs`, with the id the pack has now, on a task of its own, with a five-second timeout, and
drops whatever comes back. The service is the one sharing uses (`FOLDERSKIN_COMMUNITY_API`, or
`COMMUNITY_API` in `share.rs`), or `https://community.folderskin.app`. Development builds and
builds reading another copy of the packs send nothing unless a service is named.

## Applying an icon

`apply_skin` resolves the artwork, renders the icon set, validates the folder path, then calls
the platform writer, off the async runtime's threads (`NSWorkspace.setIcon` works off the main
thread, one call at a time, which a process-wide lock in `apply/macos.rs` makes sure of, and
the PNG encodes are slow). On macOS the old icon is cleared before the new one is set, so Finder
redraws it at once instead of showing the old one until the folder is opened. Every writer refuses anything that is not an existing directory, refuses filesystem
roots, and writes atomically. What each platform actually writes, and what revert undoes, is
in [PLATFORMS.md](PLATFORMS.md).

## A folder and its subfolders

**Include subfolders** runs the same writer over a folder's tree (`src-tauri/src/tree.rs`, with
the walk in `folderskin_core::apply::tree`), however many folders it holds.

- **Which folders.** `folders_in` lists a folder's own folders by name ignoring case. It skips,
  along with everything inside them: symlinks and junctions (and a Windows volume mounted in a
  folder, which is one), names starting with a dot, folders the OS hides (Finder's hidden flag,
  or Windows' hidden or system attribute), packages (apps, libraries and documents that are
  folders on disk, known by their extension and, on macOS, by Launch Services), the system
  locations `validate_folder` refuses, and on macOS and Linux another volume mounted inside the
  folder (a folder whose device isn't its parent's). A folder that is a package or inside one has
  nothing inside as far as a run goes.
- **Any size.** `Walk` reads a tree one folder at a time, whenever whoever drives it asks, into a
  `Tree` that keeps each folder as its name, in one buffer, and the number of the folder it's in.
  A million folders with names a dozen letters long take about 35 MB (a core test builds a tree
  that size), and a count adds 8 bytes a folder, a run 1. A run walks breadth first, so one
  that stops part way has done whole levels. Counting walks depth first, so each folder's insides
  are finished before the next folder's and their counts are final one by one.
- **Counting.** Picking a folder starts `subfolder_count` on a thread of its own, abandoning the
  count of the folder before. It answers once it's done or after 80 ms, so a small folder is
  counted before the panel shows, and a big one goes on, `subfolder-count` saying how far it has
  got at most ten times a second. Each folder in its tree keeps how many folders have been found
  inside it and how many in its tree are still to read, and `subfolder_counts` gives those for
  the folders the webview asks about.
- **Choosing some.** **Choose subfolders** (`src/components/SubfolderChooser.tsx`) reads each
  column's folders with `subfolder_list` as it opens, looking into each for folders of its own for
  a quarter of a second at most (the rest show a chevron and are read when opened), so a folder
  with any number inside opens at once, and each column draws only the rows in view. What's
  ticked is rules, not a list (`src/lib/folderChoice.ts`): ticking or clearing a folder decides it
  and everything inside it unless a rule further down says otherwise, and a rule that says what
  its folder says anyway isn't kept, so a folder with any rule inside it shows a dash without
  anything being counted. How many folders the rules take comes from the counts of the folders
  they're on: every folder as `all` says, and each rule changes its folder and everything in it
  from what the folder it's in says, which is the same sum over the folders found so far while
  the count is going.
- **One icon for all of them.** `prepare_icon` encodes the icon once (the `NSImage` on macOS, the
  `.ico` or `.png` bytes elsewhere) and `apply_prepared` puts it on each folder, checking each
  one as `apply_icon` does. On a Mac an apply takes about 110 ms a folder (it took about 610 ms
  when each folder encoded its own) and a revert about 1 ms. `bytes_per_folder` says what each
  copy costs: on macOS it is measured, by attaching the icon to a scratch folder and weighing
  its `Icon\r` (about 2.7 MB for a painting), and elsewhere it is the icon file plus a disk block
  for the text file beside it.
- **The run** (`src-tauri/src/tree/job.rs`). `start_tree_apply` and `start_tree_revert` answer at
  once, and the run goes on in the background, whatever the webview does. It is two threads over
  one record: a walker reads the tree and adds the folders the run's rules take, and a worker goes
  through them in the order they were found, starting on the folder itself straight away and
  waiting for the walker when it catches up, so a run is well under way before the count of what
  it takes is. The record keeps each folder's part in a byte beside its place in the tree (not in
  the run, to do, changed, failed with a sentence, or skipped), which is everything **Carry on**,
  **Try again** and undoing need, by the run's id: no list of folders crosses the IPC either way.
  Undoing is a revert of exactly the folders the apply changed, over the same tree. Stop is
  checked before every folder, so the folder in hand finishes. One run goes at a time: another is
  refused with a sentence while one is going, and replaces the last once that has ended. The
  webview hears `tree-run` at most twelve times a second, with the first hundred failures, and
  every message is numbered so a late one never undoes a newer one.

## Frontend state

The drop zone is a reducer in `src/state/dropzone.ts` with six phases (`idle`, `folder`,
`ready`, `applying`, `applied`, `reverting`), and it is unit-tested in isolation from React.
An error is a field on the state, not a phase, so a failed apply returns to `ready` with the
message shown beneath the button.
Components read the state and render, but do not decide transitions. Dropping a picture
selects a custom skin without changing which folder is chosen, which is why picking a folder
and picking a skin are separate axes in the machine. A folder that replaces another is
`arriving`: it shows its own icon (fetched for that folder alone, so a late answer for the one
before can't land on it) until `arrived`, about a second after the icon shows, and only then
tries the selected skin on, so the switch can be seen. Picking a skin or applying ends it early.
A folder that already wears a custom icon (`folder_icon` reports it, from
`folderskin_core::apply::has_custom_icon`) waits instead, offering **Try on** or **Remove custom
icon**, and a revert can start from there or from `folder` as well as from `applied`.

The folders inside the chosen one are counted when it's picked (`subfolders`: the count so far,
and whether it's done), and `includeSubfolders` is the **Include subfolders** switch, off again
for every new folder, and on while the count is still going as soon as it has begun. With it on,
an apply or a revert is a run over the tree, which lives outside the panel: `src/state/treeRun.ts`
keeps the latest run as the app tells of it, keeping only the newest message, and hears each one
that was going end. The panel follows the run while its folder is on show (`runId`,
`treeRunning` and `treeEnded`), and picking its folder again, or clicking the run at the foot of
the sidebar (`RunDock.tsx`, on every view, one row in a window shorter than the one FolderSkin
opens in), brings it back. The summary, its **Carry on** and **Try again**, and a **Revert all**
that takes off exactly what was put on, name the run by its id. Until a run's own walk has found
every folder, its count reads the total of the folder's count when that had finished first
(`treeRuns.expect`), rather than "3,120 of 12,000+". A run that stopped before changing anything
leaves the folder's phase as it was. A run that ends while the window isn't in front says so as
a system notification (`src/lib/notify.ts`), with permission asked for the first time a run
starts rather than as the app opens. `chosen` is the rules ticked in **Choose subfolders** when
they aren't every folder, and how many folders they take, kept up to date as the count goes. It
stays while the same folder is chosen, whether the switch is on or off, and `insideCount`,
`insideCounted` and `treeChoice` turn it into the counts the buttons show and the rules a run is
given.

The library's filters are pure functions in `src/lib/filters.ts`: the sidebar's view, then the
filters (where a skin came from, its pack, colours, brightness, when it was added, the AI model,
author and licence), then the tag tabs, the search and the sort order. A filter's counts are
worked out with the other filters as they are, and a filter that can't narrow what's in view
isn't offered. Colours and brightness come from each skin's own picture (`src/lib/palette.ts`),
read a few at a time in the background and remembered by skin id.

The composer keeps its design in a reducer too (`src/composer/history.ts`): every change is a new
document, a drag is one step once the pointer lets go, and a run of changes to the same setting
merges into one, so undo goes back by what a person would call a step. Once opened it stays
mounted, hidden while another view is shown, so a design survives a visit to the library. The
design is also kept in the webview's storage until it is saved or replaced. The folder panel
steps aside while it is open, and **Save & apply** goes through the same reducer actions as the
panel's own Apply.

Per the project style rule there are no CSS outlines, focus rings or selection outlines
anywhere. Focus and selection are shown with a background tint or a border colour change. The
composer's selected layer follows it too: a tint over the layer with knobs to size and turn it,
never a frame.

## Testing

| layer | what is tested |
|---|---|
| `geometry` | every part inside the canvas, symmetry, path closure |
| `fit` | cover-fit always covers, focus extremes keep the expected corner |
| `compositor` | determinism (same input, identical bytes) and silhouette checks against the measured constants at sample rows and columns |
| `ico` | round-trip of the multi-size container |
| `apply` | `desktop.ini` and `.directory` generation and revert parsing as pure functions, and path validation |
| `pack` | the pack contract: fields, limits, tags, ids, file names, picture checks and the pack hash. An index's optional fields, and fields it doesn't know passed over |
| `matte` | keying, despill and trim, and telling a finished folder (transparent, keyed, keyed then JPEG-compressed, trimmed tight) from an ordinary photo, a pink sunset and a product shot on magenta paper |
| `compositor` (composer) | the template's layers stacked around a design equal the saved icon, a design lands where it was drawn, and a see-through design leaves only the paper and the rims |
| `composer` | the raw body's framing, the picture checks, previews, saving a design and saving over one, a damaged document |
| `store` | round trip across a restart, one entry per picture, delete, a corrupt or missing index, damaged entries, thumbnail repair, the size bound, id checks. A batch saved in its own order, whole or not at all (a write that fails part way, an index that can't be written), saved and repeated pictures. What a crash left removed on open, and nothing else |
| `community` | a pack saved in its own order with its progress, a picture listed twice, a pack that can't be saved adding nothing, updates, and the download: four at a time, in order, with its progress, against a local server. Official packs from `index.json` and `head.json`, and a link's pack looked for again past the caches |
| `deep_link` | which links are install links and which are ignored. A good one waiting for the webview, which is told, and taken once |
| `installs` | the count's address and when one is sent at all, and the request itself (a bare POST) against a local server |
| `onboarding` | when it shows, what forces it, and the marker |
| `folderskin-tools` | the pack checks and the index (with `official.json` and each pack's date), making a pack (the split, `--flat-backdrop`, lossless WebP), the picture split `render` and `apply` use |
| frontend | the drop-zone reducer, favourites and platform copy. The composer's document, undo, geometry, text layout, shapes, colours, picture adjustments and templates. The Community store, install links included (vitest) |

The Windows writer is compile-checked from macOS with `cargo check --target
x86_64-pc-windows-msvc -p folderskin-core`. CI runs the whole set on ubuntu-22.04,
windows-latest and macos-latest.

## The AI assistant

`crates/folderskin-ai` talks to the AI providers, and only when the user presses Generate. It holds the provider catalogue, the per-provider requests and response
readers (pure functions, unit-tested without a network, and sent for real to a stand-in server
on localhost by `tests/providers.rs`) and the prompt templates. The app crate
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
740 KB of pictures (26 folders, each drawn at the size it's shown at, and the logo). Once FolderSkin stopped shipping skins the binary measured 7.0 MB and the
bundle 8.5 MB. With 2.3 MB of built-in skins they had been 8.7 MB and 10.2 MB. The updater
added 0.25 MB, to a 7.3 MB binary and an 8.8 MB bundle. The composer added 0.16 MB to the
binary and Include subfolders 0.05 MB, to a 7.6 MB binary and an 8.9 MB bundle. The composer's
script is a chunk of its own (114 KB, 41 KB gzipped) that the webview loads the first time it's
opened, and the AI view's is too (19 KB). libwebp, built in for the lossless WebP that pack
pictures and the library's pictures are saved as, added 0.18 MB of code to the binary. That keeps the first screen's script under
500 KB (488 KB, 161 KB gzipped), the line Vite warns at. Keep it there. `image` is built with `default-features = false` and
only `png`, `jpeg` and `webp`, and the release profile uses `opt-level = "s"`, LTO and one
codegen unit. Any dependency that would move this budget needs a reason in the pull request.

Vite copies everything in `public/` into every build, and Tauri embeds the build in the binary,
so dev-only files stay out of `public/`. The browser mock (`src/lib/devMock.ts`) is only used
under `import.meta.env.DEV`, so a build leaves it out, and it shows real pack pictures fetched
from folderskin-community on GitHub rather than files in this repository. Skin previews
that once sat in `public/` added 2.3 MB to the binary without showing up as files in the bundle.
