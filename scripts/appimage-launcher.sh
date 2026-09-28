#!/usr/bin/env bash
# Lets every user of a Linux machine start the AppImage, not only its owner.
#
#   scripts/appimage-launcher.sh seed                  # before `tauri build`
#   scripts/appimage-launcher.sh check <AppImage>...   # after it
#
# Tauri's bundler downloads the launcher it puts at the root of the AppImage (AppRun, which
# linuxdeploy renames AppRun.wrapped) and saves it with mode 770: its owner and group may run it,
# nobody else. Started the usual way, an AppImage's files belong to whoever starts it, so that
# goes unnoticed. Mounted by root and run by someone else, as firejail and the AppImage catalog's
# test do, it stops at "AppRun.wrapped: Permission denied". The x86_64 linuxdeploy keeps the 770;
# the ARM64 one happens not to.
#
# `seed` puts the same launcher in the bundler's cache with mode 755 first. The bundler downloads
# it only when it's missing, so it copies that one in. `check` opens each AppImage and fails when a
# file in it isn't readable by everyone, or is runnable by its owner but not by everyone, so a
# Tauri that keeps its launcher somewhere else fails the build instead of shipping.
set -euo pipefail

# Where tauri-bundler's AppImage tools live when bundle.useLocalToolsDir is off:
# dirs::cache_dir()/tauri, named after the first part of the target triple.
seed() {
  local arch dir
  arch="$(uname -m)"
  dir="${XDG_CACHE_HOME:-$HOME/.cache}/tauri"
  mkdir -p "$dir"
  curl -fsSL --retry 3 -o "$dir/AppRun-$arch" \
    "https://github.com/tauri-apps/binary-releases/releases/download/apprun-old/AppRun-$arch"
  chmod 755 "$dir/AppRun-$arch"
  echo "AppImage launcher for $arch: $dir/AppRun-$arch, mode 755"
}

check() {
  local appimage name work bad status=0
  (($#)) || { echo "check: name the AppImages to check" >&2; return 2; }
  for appimage in "$@"; do
    name="$(basename "$appimage")"
    if [ ! -f "$appimage" ]; then
      echo "::error::no AppImage at $appimage"
      status=1
      continue
    fi
    work="$(mktemp -d)"
    (cd "$work" && "$(realpath "$appimage")" --appimage-extract >/dev/null)
    # Files everyone can read, and run whenever their owner can; folders everyone can open.
    bad="$(cd "$work/squashfs-root" && find . \
      \( -type f \( ! -perm -o+r -o \( -perm -u+x ! -perm -o+x \) \) \) \
      -o \( -type d ! -perm -o+rx \) | sort | xargs -r -d '\n' ls -ld)"
    rm -rf "$work"
    if [ -n "$bad" ]; then
      echo "::error::$name: files only their owner or group can use; mounted by root and started by anyone else, the app won't open"
      printf '%s\n' "$bad"
      status=1
    else
      echo "$name: every file readable by everyone, and runnable by everyone where runnable at all"
    fi
  done
  return "$status"
}

case "${1:-}" in
  seed) seed ;;
  check) shift; check "$@" ;;
  *) echo "usage: $0 seed | check <AppImage>..." >&2; exit 2 ;;
esac
