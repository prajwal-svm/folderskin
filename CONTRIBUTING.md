# Contributing

Thanks for looking. FolderSkin is small on purpose, so the most useful contributions are bug
reports with a platform and a folder path, fixes for the per-OS icon writers, and skins.

## Getting set up

You need [Rust](https://rustup.rs) stable (edition 2021), Node 20 or newer, and pnpm 10 or
newer. Platform prerequisites are listed in the README's [building from
source](README.md#building-from-source) section; on Debian or Ubuntu that is:

```sh
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Then:

```sh
pnpm install
pnpm tauri dev
```

The first Rust build takes a few minutes. After that, edits to `src/` hot-reload and edits to
Rust restart the app.

## The checks

Run these before opening a pull request. CI runs exactly the same list on ubuntu-22.04,
windows-latest and macos-latest, so a green local run is a good predictor:

```sh
cargo test --workspace
pnpm test
pnpm typecheck
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
pnpm tauri build            # optional locally, always in CI
```

If you touch the Windows writer from another OS, also run:

```sh
rustup target add x86_64-pc-windows-msvc
cargo check --target x86_64-pc-windows-msvc -p folderskin-core
```

## Where things live

[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) is the map. In short: the folder template, the
compositor and the per-OS writers are in `crates/folderskin-core`; the app's commands are in
`src-tauri/src/commands.rs`; the webview is in `src/`; the maintainer CLI is
`crates/folderskin-tools`.

## House rules

These are the constraints that keep the app what it is. A pull request that breaks one will
get a request for changes, not a silent merge.

1. **One rendering path.** Every pixel comes from `folderskin_core::compositor`. Do not draw
   folder geometry in CSS, SVG or canvas, even for a thumbnail — the webview displays PNGs the
   Rust side rendered.
2. **No CSS outlines.** No `outline`, no focus ring, no selection outline anywhere in the UI.
   Show focus and selection with a background tint or a border colour change.
3. **Geometry constants are the source of truth.** The numbers in
   `crates/folderskin-core/src/geometry.rs` were measured from a real applied icon and are
   covered by silhouette tests. Change them only with a reason and updated tests.
4. **Errors are sentences.** Commands return plain strings that the drop zone shows verbatim,
   in the app's voice ("couldn't apply the skin: permission denied"). No error codes, no Rust
   `Debug` output reaching the user.
5. **No network, no telemetry, no accounts.** The app must work with the machine offline and
   must not phone home. A dependency that opens a socket will not be merged.
6. **Keep the size budget.** Under 15 MB installed. New dependencies need a sentence in the
   pull request explaining why, and `image` stays on `default-features = false`.
7. **Only clean assets.** Art in the repository must be original or CC0, with `author` and
   `license` filled in. Never commit anything extracted from another product.
8. **Tests come with behaviour.** Pure functions — geometry, cover-fit, `desktop.ini` and
   `.directory` generation and parsing, the manifest, the drop-zone reducer — are all unit
   tested, and new behaviour in them should arrive with its test. Write the test first if you
   can.

## Adding or changing a skin

[docs/SKINS.md](docs/SKINS.md) has the format, the safe areas and the commands.
[.claude/skills/folderskin-skins/SKILL.md](.claude/skills/folderskin-skins/SKILL.md) is the
same workflow written for Claude Code.

Two things catch everybody: skins are **embedded at build time**, so you must rebuild to see a
change, and the shipped set is **exactly ten** — replace a skin rather than adding an
eleventh, unless a maintainer has agreed otherwise. Include the preview in your pull request
so reviewers can see the composition without building.

## Working on the icon writers

The writers are the part most likely to be wrong on a machine you do not own, so please say in
the pull request which OS and file manager you tested on. Two things make this easier:

- The pure content generators (`desktop.ini` and `.directory` text, and the revert parsers) are
  unit-testable without touching a filesystem. Add cases there first.
- `folderskin-tools apply <folder> --skin aurora` and `folderskin-tools revert <folder>` drive
  the writers from a terminal, so you can test on a headless VM with no GUI.

Every writer must validate the path, refuse filesystem roots, write atomically, remove only
what FolderSkin wrote, and be safe to run twice.

## Pull requests

- One topic per pull request. A refactor and a fix in the same branch are two pull requests.
- Commit messages: imperative and sentence case, e.g. `Fix desktop.ini revert when the user
  added their own keys`. Explain *why* in the body when it is not obvious.
- Add a line to the `Unreleased` section of [CHANGELOG.md](CHANGELOG.md) for anything a user
  would notice.
- Update the docs in the same pull request as the behaviour. [docs/PLATFORMS.md](docs/PLATFORMS.md)
  should always describe what the writers actually do.
- Screenshots for anything visual, please: light window, default window size.

## Reporting a bug

Use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.md) and include your OS
version, the app version, the file manager, and whether the folder was on a local disk, a
network share or a cloud-synced directory. That last one explains a surprising number of
reports.

Security problems go through [SECURITY.md](SECURITY.md) instead, not the issue tracker.

## Licence

FolderSkin is MIT. By contributing you agree that your contribution is released under the MIT
licence and that you have the right to release it.
