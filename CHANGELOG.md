# Changelog

All notable changes to FolderSkin are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.1 — 2026-09-19

### Added

- **Design your own**: a composer for making a skin yourself, offline. It starts from one of
  sixteen templates (plain, a colour, a label, an emoji, a tab label, two-tone, glass, tinted
  glass, stripes, gingham, polka dots, sunset, neon, a badge, a photo with a caption, a sticker).
  From there it takes:
  - any colour with transparency, and linear and radial gradients
  - text in seventeen font styles, with spacing, curves and an outline
  - emoji, thirteen shapes, and ten patterns including confetti and film grain
  - your pictures (from a file, your skins, a drop or the clipboard) with adjustments
  - shadows, glows, sticker edges, opacity and blend modes, on as many layers as you like, with
    undo

  The **Folder skeleton** switch shows the design on the folder or flat with the folder's edges,
  and it can be seen on a light, dark or colourful desktop and at the sizes Finder draws.
  **Free icon** makes the design the whole icon, any shape. **Save & apply** puts it on the chosen
  folder, **Edit design** opens a saved design again, and any other skin can be remixed. See
  [docs/COMPOSER.md](docs/COMPOSER.md).
- `folderskin-tools composer-layers` writes the folder template's layers the composer draws a
  design between.
- **Include subfolders**: a switch under the chosen folder puts the skin on that folder and every
  folder inside it, all levels down, and says how many that is before anything changes. Runs over
  more than ten folders ask first, with roughly the space the icons take (each folder keeps its
  own copy). A run shows how far it has got, can be stopped, and ends with what happened: how many
  folders changed, which couldn't be and why, with **Carry on** after a stop and **Try again** for
  failures. **Revert** takes off exactly what the run put on; with the switch on and no run to
  undo, **Remove custom icons** clears the whole tree after asking. Hidden folders, app bundles and
  other packages, symlinks and system locations are left alone, and a tree of more than 5,000
  folders is refused. The composer's **Save & apply** follows the same switch.

### Changed

- The composer's **Layers** and the settings below them are each opened and closed from their
  heading, and the bar between them is dragged (or nudged with the arrow keys) to share out the
  height; how they were left is remembered. A layer can be deleted from its own row, and
  **Delete all** clears the design, with **Undo** in the toast. A heading's buttons, like a row's,
  show when it is pointed at, and stay reachable from the keyboard.
- The sidebar's **Create** group holds **Design your own** and **Generate with AI**; **Community**
  is under **Explore**.
- A pack can be shared from designs made in the composer too.
- A skin's ⋯ menu shows its name as a labelled field with a pencil, so it's clear it can be
  renamed there. A double click on a name in the gallery opens it ready to type over, as F2 does.
  Only your own skins can be renamed: a skin from a community pack keeps the name it was shared
  under, and shows its tags and details as before.
- Long names are cut short with an ellipsis everywhere they appear, with the full name on hover:
  in the folder panel, gallery, dialogs, toasts, buttons and the composer.
- Disabled buttons and switches show the not-allowed cursor.
- The browser stand-ins used by `pnpm dev` are left out of release builds, and the AI view loads
  the first time it's opened, which keeps the first screen's script under 500 kB.

### Fixed

- The emoji picker scrolled sideways. Every list, grid, panel and dialog in the app now shows a
  scrollbar only while the pointer is over it, instead of a few of them.
- Sliders showed the text cursor instead of the hand.
- **You're up to date** is now plain text with the same green tick as everywhere else, rather than
  green writing.
- On macOS, two icon changes at once (an apply starting while another was still being written)
  could garble each other's icon or fail. Changes now go one at a time.
- F2 on a skin opened its menu without putting the cursor in the name field.
- A dropdown lost the chevron drawn on it while the pointer was over it.
- Typing a number into a box applied a clamped value with every keystroke, so typing 50 into a
  field that allows 10 to 100 jumped to 10 and moved the design with it. It now applies what is
  already in range while you type, and clamps when you leave the box.
- A switched-on row, such as **Include subfolders**, showed nothing when hovered or focused, so
  there was no sign of where the keyboard was.
- A button's icon no longer shrinks to a sliver beside a long label.

## 0.1.0 — 2026-09-19

The first release.

### Added

- Apply a folder icon on macOS, Windows and Linux from one rendering path in
  `folderskin-core`, so the gallery preview and the icon written to disk are the same
  pixels. On macOS the Desktop and Finder show the new icon straight away, even when it
  replaces another.
- A first-launch welcome: five folders try on 26 skins, none of them twice and no two neighbours
  from one pack (paintings, pop-art portraits and marble statues, with a couple of plain folders),
  fold into one and land on the logo, whose **Let's go** arrow nudges until it's pressed. Then the
  community's packs are offered with Classic Art picked for you, going soft and fading out under
  frosted glass as they scroll down to the buttons. Each pack shows its progress, nothing can be
  skipped or undone while one is being added, and a pack that fails can be tried again or left for
  later from Community. It shows once; a click or a key skips the animation, and reduced motion
  shows its last frame straight away.
- FolderSkin ships no skins of its own: the library holds community packs, your own pictures
  and AI results, and an empty library points to all three.
- Use your own photo as a skin: drop a picture on the window, or pick one with the
  "your photo" button.
- An optional AI assistant that makes skins with your own API key for OpenAI, xAI,
  Recraft, Google, Black Forest Labs, Stability AI or Ideogram. Keys are encrypted on your
  computer (AES-256-GCM, tied to that computer) in a file only your account can read, so there
  are no keychain password prompts. See [docs/AI.md](docs/AI.md).
- Revert, which restores the operating system's default folder icon and removes the
  files FolderSkin wrote. A folder that already wears a custom icon when you pick it offers
  **Remove custom icon** straight away, and waits for you to try a skin on instead of doing it
  for you.
- Animated icons, adapted from lucide-animated, that play when their button is hovered or
  focused.
- Everything you add is saved: pictures you import and every AI result come back after a
  restart under **Yours**. Deleting one asks first.
- Tags on every skin. The tags in view become the filters along the top of the library, AI
  results are tagged with their style, and a skin's ⋯ menu renames it, edits its tags and shows
  how it was made.
- Filters and a sort order behind a button beside the search: favourites only, where a skin came
  from, its pack, its colours and whether it's light or dark (read from the picture itself), when
  it was added, and the AI model, author or licence behind it. Only the filters that can narrow
  what's in view are offered, each saying how many skins it would show, and the button counts the
  ones that are on. The search also finds a skin by its pack, its author or the idea behind an AI
  result.
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
  apply, revert) has its own feedback. A different folder shows up as it is first, wearing its
  own icon, and then tries the chosen skin on.
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
- A Greek Art community pack: 15 marble statues and temples as folders, from Open Arms and the
  Parthenon to Poseidon in the Clouds.
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
