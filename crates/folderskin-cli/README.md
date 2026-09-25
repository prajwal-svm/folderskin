# folderskin, the command line

FolderSkin from a terminal. Paint folder art with a model on your own computer or with your own
API key, clean pictures up, put them on folders and take them off again, and make community packs.
It does what the app does, and it's handy for batches and whole drives.

## Install

On a Mac or Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/prajwal-svm/folderskin/main/scripts/install-cli.sh | sh
```

On Windows, in PowerShell:

```powershell
irm https://raw.githubusercontent.com/prajwal-svm/folderskin/main/scripts/install-cli.ps1 | iex
```

Either one downloads the newest command-line release, checks it against its SHA-256, and puts
`folderskin` on your PATH: in `~/.local/bin` on a Mac or Linux, and in
`%LOCALAPPDATA%\Programs\folderskin` on Windows. You don't need admin rights.
`FOLDERSKIN_VERSION=0.1.0` installs a particular version, and `FOLDERSKIN_INSTALL_DIR` puts it
somewhere else.

The command line has releases of its own, tagged `cli-v…`, apart from the app's. If you download
an archive in a browser instead, macOS may refuse to open what's inside, so use the installer.

## Things to try

```sh
# What this computer can run, and what `ai gen` would use
folderskin ai doctor

# Set up the Local Model once: 4.6 GB on a Mac, 5.2 GB elsewhere
folderskin ai setup

# Paint four takes on one idea
folderskin ai gen "a lighthouse at dusk" --style woodblock -n 4

# Paint every folder in Projects from its own name, in one style, and apply them
folderskin ai theme ~/Projects --style travel-poster --apply

# Put a picture on a folder, then give the folder its own icon back
folderskin apply ~/Music --image vinyl.webp
folderskin revert ~/Music

# See a picture as the folder FolderSkin makes of it
folderskin render photo.jpg --out folder.png
```

## Commands

| Command | What it does |
|---|---|
| `ai` | Paints folder art: `doctor`, `setup`, `gen`, `batch` (a JSON file of ideas), `theme`, `styles`, `models`, `config`, `key` |
| `image` | Gets a picture ready: `crop`, `trim`, `clip`, `cutout`, `check`, colour adjustments, `info` |
| `apply`, `revert` | Puts a picture on a folder, and gives the folder its own icon back |
| `render`, `template` | Draws a picture as the app's folder icon, and writes the blank folder a model repaints |
| `packs` | Makes, checks and indexes community packs ([the packs guide](../../docs/PACKS.md)) |

Every command explains itself with `--help`. `--json` writes each result, progress update and
error as one JSON object per line, for scripts. `--look mac` or `--look windows` picks the folder
the artwork goes on. Without it you get the one chosen in the app.

## Building it yourself

`cargo build --release -p folderskin-cli` builds `target/release/folderskin-cli`. The app's own
binary is already called `folderskin`, so the release archives rename this one to that.
