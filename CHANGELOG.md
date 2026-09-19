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
- A first-launch welcome: five folders try on skins from the community packs, mostly paintings
  and pop-art portraits with no two neighbours from one pack, fold into one and land on the logo,
  whose **Let's go** arrow nudges until it's pressed. Then the community's packs are offered with
  Classic Art picked for you. Each pack shows
  its progress, nothing can be skipped or undone while one is being added, and a pack that fails
  can be tried again or left for later from Community. It shows once; a click or a key skips the
  animation, and reduced motion shows its last frame straight away.
- FolderSkin ships no skins of its own: the library holds community packs, your own pictures
  and AI results, and an empty library points to all three.
- Use your own photo as a skin: drop a picture on the window, or pick one with the
  "your photo" button.
- An optional AI assistant that makes skins with your own API key for OpenAI, xAI,
  Recraft, Google, Black Forest Labs, Stability AI or Ideogram. Keys are encrypted on your
  computer (AES-256-GCM, tied to that computer) in a file only your account can read, so there
  are no keychain password prompts. See [docs/AI.md](docs/AI.md).
- Revert, which restores the operating system's default folder icon and removes the
  files FolderSkin wrote.
- Animated icons, adapted from lucide-animated, that play when their button is hovered or
  focused.
- Everything you add is saved: pictures you import and every AI result come back after a
  restart under **Yours**. Deleting one asks first.
- Tags on every skin. The tags in view become the filters along the top of the library, AI
  results are tagged with their style, and a skin's ⋯ menu renames it, edits its tags and shows
  how it was made.
- Community packs: free skins shared on GitHub under `community/packs/`, added from the
  Community view and filtered by tag. Share one skin or a pack of up to 50 from the app, which
  saves a folder ready for a pull request. The contract, its limits and the licences (CC0, CC BY
  4.0, MIT) are in [docs/PACKS.md](docs/PACKS.md); `folderskin-tools packs check` runs the same
  checks in CI, and a workflow publishes the list and previews.
- Settings: theme, AI keys, sharing defaults and where skins are saved, in one dialog. About
  opens from the version badge beside the logo, with GitHub in its corner (View Source), Report
  issues and Star project side by side, and a check for updates.
- Finished folder pictures on a flat magenta background, such as ones painted in Grok's or
  ChatGPT's chat from [docs/PROMPTS.md](docs/PROMPTS.md), are cut out and used as they are.
- A layout built around the folder: the library and the folder each sit on their own island,
  the folder tries skins on before anything is written, and every step (drag, drop, preview,
  apply, revert) has its own feedback.
- The AI assistant is a composer that moves out of the way once used; results develop in
  place and can be tried on straight away, and provider settings live in a dialog.
- A translucent sidebar on macOS: the window sits on the system sidebar material, which
  follows the light/dark switch.
- Favourites, stored locally.
- The sidebar shows how many skins All skins, Yours and Favourites hold, and nothing for none.
- `folderskin-tools`, a workspace binary that makes, checks and indexes packs, renders any
  picture as the folder it makes, draws the template's safe areas, and applies or reverts an
  icon from a terminal. See [docs/SKINS.md](docs/SKINS.md).
- A Claude Code skill for making packs at
  [.claude/skills/folderskin-skins/SKILL.md](.claude/skills/folderskin-skins/SKILL.md).
- `folderskin-tools packs make` turns a folder of pictures, such as renders from an image model,
  into a community pack: finished folders on magenta or transparency are cut out and saved as
  WebP, everything else as artwork, each within 400 KB. See [docs/PACKS.md](docs/PACKS.md).
- A pack is added whole or not at all: its pictures download four at a time and are all checked,
  then saved in one step, so a dropped connection, a full disk or a crash never leaves half a
  pack, and files a crash left behind are cleared on a later launch. Adding one reports its
  progress, and its skins keep the pack's order in the library. See
  [docs/PACKS.md](docs/PACKS.md).
- A Classic Art community pack: sixteen public-domain paintings, from the Mona Lisa to
  Composition VIII, each painted onto a folder.
- A Soft Rainbow community pack: ten pastel folders, from Lemon Chiffon to Celadon.
- A Scientists - Pop Art community pack: 42 scientists as comic-book folders, from Archimedes to
  Stephen Hawking, in the order they were born.
- Community opens on a gallery of cards with four of each pack's folders, with a list a click
  away, and a pack you've added wears a green tick by its name. **View** opens a pack to look
  through all of its skins before adding it; one looked through in the last week opens straight
  away from a copy kept on this computer, drawn again whenever the pack changes. **Refresh**
  reads the list from GitHub again, and a pack that changed since you added it offers
  **Update**.
- `packs make --flat-backdrop` for renders whose magenta drifted to pink or raspberry: it
  removes each picture's own flat background where it reaches the edge, drop shadow included,
  and splits the edge pixels back into painting and background so no pink rim is left. On a
  plain grey or black background it keeps to that background's own noise and never grows
  upward, so dark clothes and ink lines on the folder's edge stay.
- The AI providers' own logos (from lobe-icons) on their key tiles and on the studio's model
  button, and animated icons on the appearance choices and the About links. The Settings tabs
  are centred.
- Installers for macOS (one universal app, signed and notarized), Windows, and Linux on both
  x86_64 and ARM64, built as a draft release from a tag and published by hand once tried. See
  [docs/RELEASING.md](docs/RELEASING.md).
- FolderSkin updates itself. A few seconds after it opens it looks for a newer release on
  GitHub; if there is one it shows what changed and, on **Update and restart**, downloads it,
  checks its signature against the key built into the app, installs it and restarts. About and
  Settings → About check on request, and the version badge wears a dot while an update waits.
- A release build behaves like an app, not a web page: no right-click menu, no web inspector,
  and the reload and inspector shortcuts do nothing. Development builds keep them.

### Notes

- No accounts, no paywall, no telemetry. The only network access is the optional AI
  assistant, which calls the provider you pick directly; Community and the first-launch
  welcome, which read the shared packs from GitHub; and the update check, which reads the
  newest release's `latest.json` from GitHub when the app opens.
- The macOS app is 8.8 MB installed. The DMG and the Windows and Linux packages have not been
  measured yet.
