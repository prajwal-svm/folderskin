#!/bin/sh
# Installs the `folderskin` command line on macOS or Linux, from its newest release (a cli-v*
# tag; the app's releases are separate), after checking the download against its SHA-256.
#
#   curl -fsSL https://raw.githubusercontent.com/prajwal-svm/folderskin/main/scripts/install-cli.sh | sh
#   FOLDERSKIN_VERSION=0.1.0 sh install-cli.sh             # a particular version
#   FOLDERSKIN_INSTALL_DIR=/usr/local/bin sh install-cli.sh # somewhere else (default ~/.local/bin)
#
# POSIX sh, so it runs where bash doesn't exist. Every failure says what to do next.
set -eu

REPO="prajwal-svm/folderskin"
VERSION="${FOLDERSKIN_VERSION:-}"
DEST="${FOLDERSKIN_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf 'folderskin: %s\n' "$*" >&2; }
fail() {
  printf 'folderskin: %s\n' "$1" >&2
  if [ -n "${2:-}" ]; then printf '  Try: %s\n' "$2" >&2; fi
  printf '  Or ask Claude: claude "The folderskin installer failed on %s %s: %s Help me install it."\n' \
    "$(uname -s)" "$(uname -m)" "$1" >&2
  exit 1
}

os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Darwin) platform="macos-universal" ;;
  Linux)
    case "$arch" in
      x86_64 | amd64) platform="linux-x86_64" ;;
      aarch64 | arm64) platform="linux-aarch64" ;;
      *) fail "There is no folderskin build for Linux on $arch." "Build it from source: cargo install --path crates/folderskin-cli" ;;
    esac
    ;;
  *) fail "This installer is for macOS and Linux." "On Windows, run scripts/install-cli.ps1 in PowerShell." ;;
esac

# Something to download with, and something to check with.
if command -v curl >/dev/null 2>&1; then
  get() { curl -fsSL --retry 3 -o "$2" "$1"; }
  get_text() { curl -fsSL --retry 3 "$1"; }
elif command -v wget >/dev/null 2>&1; then
  get() { wget -q -O "$2" "$1"; }
  get_text() { wget -q -O - "$1"; }
else
  fail "Neither curl nor wget is installed." "Install curl with your package manager, then run this again."
fi
if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
  fail "There is no sha256sum or shasum to check the download with." "Install coreutils, then run this again."
fi
command -v tar >/dev/null 2>&1 || fail "tar isn't installed." "Install tar with your package manager, then run this again."

if [ -z "$VERSION" ]; then
  # The app's releases share the repository, so the newest command line is the first cli-v tag.
  tag="$(get_text "https://api.github.com/repos/$REPO/releases?per_page=100" 2>/dev/null |
    grep -o '"tag_name": *"cli-v[^"]*"' | head -n 1 | sed 's/.*"\(cli-v[^"]*\)"/\1/')" || true
  [ -n "${tag:-}" ] || fail "Couldn't find a folderskin release on GitHub." \
    "Check the internet connection, or name a version: FOLDERSKIN_VERSION=0.1.0 sh install-cli.sh"
else
  tag="cli-v${VERSION#v}"
fi
version="${tag#cli-v}"
asset="folderskin-cli-$version-$platform.tar.gz"
base="https://github.com/$REPO/releases/download/$tag"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM
say "downloading $asset"
get "$base/$asset" "$tmp/$asset" || fail "Couldn't download $asset from $base." \
  "Check the version exists at https://github.com/$REPO/releases, then run this again."
get "$base/$asset.sha256" "$tmp/$asset.sha256" || fail "Couldn't download the checksum for $asset." \
  "Run this again in a moment."
want="$(cut -d' ' -f1 <"$tmp/$asset.sha256")"
have="$(sha256 "$tmp/$asset")"
[ "$want" = "$have" ] || fail "The download doesn't match its published SHA-256, so nothing was installed." \
  "Run this again; if it happens twice, report it at https://github.com/$REPO/issues"
tar -xzf "$tmp/$asset" -C "$tmp" || fail "The archive couldn't be unpacked." "Run this again."

# The FolderSkin app's Linux packages install a program called folderskin too (the window).
# Whichever comes first on the PATH runs, so say so rather than surprise anyone.
existing="$(command -v folderskin 2>/dev/null || true)"
if [ "$os" = "Linux" ] && [ -n "$existing" ] && [ "$existing" != "$DEST/folderskin" ]; then
  say "warning: $existing is already called folderskin (the FolderSkin app installs one)."
  say "  The command line goes in $DEST; put that earlier on your PATH, or run it as $DEST/folderskin."
fi

mkdir -p "$DEST" || fail "Couldn't make $DEST." "Pick a folder you can write to: FOLDERSKIN_INSTALL_DIR=... sh install-cli.sh"
cp "$tmp/folderskin" "$DEST/folderskin.new" && chmod 755 "$DEST/folderskin.new" &&
  mv -f "$DEST/folderskin.new" "$DEST/folderskin" ||
  fail "Couldn't write $DEST/folderskin." "Pick a folder you can write to: FOLDERSKIN_INSTALL_DIR=... sh install-cli.sh"

# On the PATH from the next shell on, if it isn't already.
case ":$PATH:" in
  *":$DEST:"*) ;;
  *)
    case "${SHELL:-}" in
      */zsh) rc="$HOME/.zshrc" ;;
      */bash) if [ "$os" = "Darwin" ]; then rc="$HOME/.bash_profile"; else rc="$HOME/.bashrc"; fi ;;
      *) rc="$HOME/.profile" ;;
    esac
    line="export PATH=\"$DEST:\$PATH\""
    if ! grep -qsF "$line" "$rc"; then
      printf '\n# The folderskin command line\n%s\n' "$line" >>"$rc"
      say "added $DEST to your PATH in $rc; open a new terminal, or run: $line"
    fi
    ;;
esac

say "installed folderskin $version in $DEST"
"$DEST/folderskin" --version
if [ "$platform" = "linux-aarch64" ]; then
  # stable-diffusion.cpp publishes x86_64 Linux builds only, so `ai setup` has nothing to install.
  say "note: the local models don't run on ARM64 Linux; the image tools do, and so does painting"
  say "  with your own key (folderskin ai gen \"...\" --provider openai; see folderskin ai models)."
fi
say "next: folderskin ai doctor"
