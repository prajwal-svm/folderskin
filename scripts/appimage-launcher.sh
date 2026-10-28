#!/usr/bin/env bash
# Checks that every user of a Linux machine can start the AppImage, not only its owner.
#
#   scripts/appimage-launcher.sh check <AppImage>...   # after `tauri build`
#
# Before Tauri 2.12, the bundler saved the launcher it puts at the root of the AppImage (AppRun,
# which linuxdeploy renames AppRun.wrapped) with mode 770: its owner and group could run it,
# nobody else. Started the usual way, an AppImage's files belong to whoever starts it, so that
# goes unnoticed. Mounted by root and run by someone else, as firejail and the AppImage catalog's
# test do, it stops at "AppRun.wrapped: Permission denied". The x86_64 linuxdeploy kept the 770;
# the ARM64 one happened not to.
#
# `check` lists the modes stored in each AppImage and fails when a file in it isn't readable by
# everyone, or is runnable by its owner but not by everyone, or a folder isn't open to everyone,
# so a Tauri that brings the 770 back fails the build instead of shipping. It reads the image with
# unsquashfs (squashfs-tools) rather than unpacking it: the AppImage runtime's --appimage-extract
# makes every folder it unpacks owner-only (since late September 2026), whatever the image stores.
set -euo pipefail

check() {
  local appimage name path offset listing bad status=0
  (($#)) || { echo "check: name the AppImages to check" >&2; return 2; }
  command -v unsquashfs >/dev/null || { echo "::error::check needs unsquashfs (squashfs-tools)" >&2; return 2; }
  for appimage in "$@"; do
    name="$(basename "$appimage")"
    if [ ! -f "$appimage" ]; then
      echo "::error::no AppImage at $appimage"
      status=1
      continue
    fi
    path="$(realpath "$appimage")"
    # Where the image starts, past the runtime; the runtime says so itself.
    offset="$("$path" --appimage-offset)"
    listing="$(unsquashfs -lln -o "$offset" "$path")"
    # Files everyone can read, and run whenever their owner can; folders everyone can open. Links
    # carry no modes of their own.
    bad="$(printf '%s\n' "$listing" | awk '
      $1 ~ /^-/ && (substr($1, 8, 1) != "r" || (substr($1, 4, 1) ~ /[xs]/ && substr($1, 10, 1) !~ /[xt]/)) { print; next }
      $1 ~ /^d/ && (substr($1, 8, 1) != "r" || substr($1, 10, 1) !~ /[xt]/) { print }')"
    if [ -z "$(printf '%s\n' "$listing" | grep -m1 ' squashfs-root/AppRun$')" ]; then
      echo "::error::$name: couldn't list what's in it"
      status=1
    elif [ -n "$bad" ]; then
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
  check) shift; check "$@" ;;
  *) echo "usage: $0 check <AppImage>..." >&2; exit 2 ;;
esac
