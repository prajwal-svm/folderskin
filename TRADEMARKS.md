# Trademarks

FolderSkin's code is free software under the [GNU General Public License v3.0](LICENSE). The
FolderSkin name and logo are not part of that licence. They tell people that a build is the one
published here, so they stay with the official releases. Section 7(e) of the GPL provides for
exactly this.

## Fine without asking

- Saying that your project is a fork of FolderSkin, is based on it, or works with it.
- Linking to FolderSkin or folderskin.app, and writing, teaching or making videos about it.
- Passing on the official releases unchanged, under their own name.
- Showing the logo to mean FolderSkin itself: in an article, a review, a list of apps.

## Ask first

- Releasing a changed build under the FolderSkin name or logo, or under a name or icon that could
  be taken for them.
- Naming another app, a website, a store listing or a domain so that it looks like FolderSkin's.
- Suggesting that FolderSkin endorses your project, product or service.

To ask, [open an issue](https://github.com/prajwal-svm/folderskin/issues/new).

## Releasing your own build

A changed version you give to other people needs a name and icon of its own. Change the parts that
tie a build to FolderSkin's releases and services too, so the two can live on one computer without
getting in each other's way:

- `productName` and `identifier` in `src-tauri/tauri.conf.json`, and the icons in `src-tauri/icons/`.
- The updater's `pubkey` and `endpoints` in the same file, so your users update from your releases.
- The `folderskin` link scheme there too, so install links on folderskin.app keep opening
  FolderSkin.
- The community service address, `COMMUNITY_API` in `src-tauri/src/share.rs` and `COUNTS_API` in
  `src-tauri/src/installs.rs`, unless you mean to send packs to FolderSkin's review queue and
  count adds on its gallery.
