# Changelog

All notable changes to FolderSkin are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

Nothing yet.

## 0.1.0 — unreleased

First release. The date lands here when the `v0.1.0` tag is pushed.

### Added

- Apply a folder icon on macOS, Windows and Linux from one rendering path in
  `folderskin-core`, so the gallery preview and the icon written to disk are the same
  pixels.
- Ten built-in skins in three collections: `aurora`, `sunset`, `mesh`, `ember` (glow),
  `paper`, `denim`, `slate` (grain), `halftone`, `stripes`, `bubbles` (pop).
- Use your own photo as a skin: drop a picture on the window, or pick one with the
  "your photo" button.
- An optional AI assistant that makes skins with your own API key for OpenAI, xAI,
  Recraft, Google, Black Forest Labs, Stability AI or Ideogram. Keys are saved in a
  private file only your account can read, so there are no keychain password prompts. See
  [docs/AI.md](docs/AI.md).
- Revert, which restores the operating system's default folder icon and removes the
  files FolderSkin wrote.
- Animated icons, adapted from lucide-animated, that play when their button is hovered or
  focused.
- Everything you add is saved: pictures you import and every AI result come back after a
  restart under **Yours**. You can rename them, and deleting one asks first.
- Finished folder pictures on a flat magenta background, such as ones painted in Grok's or
  ChatGPT's chat from [docs/PROMPTS.md](docs/PROMPTS.md), are cut out and used as they are.
- A layout built around the folder: the library and the folder each sit on their own island,
  the folder tries skins on before anything is written, and every step (drag, drop, preview,
  apply, revert) has its own feedback.
- The AI assistant is a composer that moves out of the way once used; results develop in
  place and can be tried on straight away, and provider settings live in a dialog.
- A translucent sidebar on macOS: the window sits on the system sidebar material, which
  follows the light/dark switch.
- Gallery tabs (`all`, the three collections, and `faves`) with favourites stored
  locally.
- `folderskin-tools`, a workspace binary that generates and imports skins, renders
  previews, checks the manifest, and applies or reverts an icon from a terminal. See
  [docs/SKINS.md](docs/SKINS.md).
- A Claude Code skill for authoring skins at
  [.claude/skills/folderskin-skins/SKILL.md](.claude/skills/folderskin-skins/SKILL.md).

### Notes

- No accounts, no paywall, no telemetry. The only network access is the optional AI
  assistant, which calls the provider you pick directly.
- The macOS app is 10.2 MB installed, from an 8.5 MB DMG. The Windows and Linux packages
  have not been measured yet.
