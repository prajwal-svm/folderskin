---
name: folderskin-skins
description: Adds, replaces and checks the skins FolderSkin ships. Crops a picture to the 1024×958 skin format around a focus point, writes the file and its manifest entry, renders a preview of the finished folder icon, judges the composition against the folder template's safe areas, adjusts the focus point and reruns until it is right, then validates the whole set and reminds the user to rebuild. Use when the user says "make a skin", "make a skin from this photo", "add a skin", "turn this photo into a folder icon", "use this picture as a folder icon", "replace the aurora skin", "regenerate skins" or "check the skins", or asks for a new built-in background for FolderSkin. Also turns a folder of renders (for example from Grok Imagine) into a pack built into the app: "make a pack from these", "add a built-in pack", "add the 3D pack".
---

# Authoring FolderSkin skins

Follow these steps in order. Run every command from the repository root — the directory that
contains `Cargo.toml`, `package.json` and `assets/skins/manifest.json`. Do not edit
`assets/skins/manifest.json` by hand; the tool keeps it in sync.

Background, if you need it: [docs/SKINS.md](../../../docs/SKINS.md).

## Step 1 — confirm the source picture

1. Get the absolute path to the picture from the user. If they have not given one, ask; do not
   guess and do not invent art.
2. Check the file exists and note its extension:

   ```sh
   ls -l "<image>"
   ```

3. Accepted inputs are PNG, JPEG and WebP. If the file is HEIC or HEIF, convert it first (this
   works on macOS only):

   ```sh
   sips -s format png "<image>" --out /tmp/in.png
   ```

   Use `/tmp/in.png` as the image for the rest of the steps.
4. Ask the user who made the picture and under what licence, unless they already said. Only
   original art or CC0 art may be committed. If the answer is "I found it online" or is
   unclear, stop and tell them the skin cannot go into the repository, but that they can drop
   the picture onto the app to use it on their own machine.

## Step 2 — choose the id, name and collection

- `id`: lowercase letters, digits and dashes only, e.g. `dunes`, `night-market`. It is also the
  preview's filename.
- `name`: what the gallery shows, title case, one or two words, e.g. `Dunes`.
- `collection`: exactly one of `glow` (luminous, gradient, dark), `grain` (textured, matte,
  paper-like), `pop` (flat, graphic, high-contrast). Pick by how the picture looks, not by what
  it depicts.

Propose the three values to the user in one line and carry on unless they object.

## Step 3 — decide whether this replaces a skin

Read the manifest and count:

```sh
cargo run -p folderskin-tools -- skin check
```

The shipped set is exactly ten skins. If the manifest already lists ten and the user has not
said to grow the set, ask which skin this one replaces, then pass that skin's `--id` in step 4
so the entry and the file are overwritten in place. Never add an eleventh skin on your own
initiative.

## Step 4 — import the picture

```sh
cargo run -p folderskin-tools -- skin add "<image>" \
  --id <id> --name <Name> --collection <collection> --focus 0.5,0.5
```

- Start at `--focus 0.5,0.5` unless the subject is obviously high or low in the frame.
- Add `--lossless` to keep PNG instead of JPEG when the art is flat colour, hard-edged shapes,
  halftone dots or stripes. Leave it off for photographs, gradients and grain.
- `--dir assets/skins` is the default; pass it only when working on a copy of the set.
- The command is idempotent per id: rerunning it with the same `--id` overwrites the file, the
  manifest entry and the preview. It never creates a duplicate.

## Step 5 — look at the preview and judge it

The command wrote `assets/previews/<id>.png`, which is the finished folder icon, not the raw
crop. **Open it with the Read tool** and check all five of these:

| check | what a failure looks like |
|---|---|
| The subject sits inside the front panel | a head, horizon or logo cut in half by the top edge of the front panel |
| Nothing important is in the top 12% of the source | the interesting part of the picture is buried in the narrow tab, or is sliced by the cream paper strip |
| The front panel's crop keeps the subject whole | the subject's top or bottom is clipped by the panel edge (the front panel keeps only the middle ~85% of the image height) |
| The corners are quiet | detail that matters is lost to the 55 px corner radius |
| No letterboxing or edge artefacts | a band of flat colour along an edge, a seam, or a stretched-looking edge from an upscaled source |

Say out loud which check failed before changing anything. If all five pass, go to step 7.

## Step 6 — adjust the focus point and rerun

Change `--focus` and rerun the exact command from step 4. Then repeat step 5. Two or three
iterations is normal.

- The `y` value is the one that does the work. The front panel keeps 838 of the 958 source
  rows, so the whole travel is 120 px: `0.1` of focus y moves the crop by about 12 px.
- Subject clipped at the **top** → lower `y` towards `0` (keeps more of the image's top).
- Subject clipped at the **bottom**, or too much empty sky above it → raise `y` towards `1`.
- The `x` value barely matters: the front panel is fitted to its width, so `x` only shifts the
  back panel's 18 px side trim. A subject that is off-centre horizontally cannot be fixed with
  focus — crop the source picture yourself and rerun step 4.
- If no focus value keeps the subject whole, the picture is the wrong shape for a folder. Tell
  the user, and offer to crop it to 1024 × 958 yourself before importing.

To see where the template's regions fall on a source image, write the guide and open it:

```sh
cargo run -p folderskin-tools -- skin guide --out /tmp/guide.png
```

## Step 7 — validate the set

```sh
cargo run -p folderskin-tools -- skin check
```

Fix every problem it reports before moving on. It exits non-zero on a problem and it checks
ids, duplicates, missing files, dimensions (must be 1024 × 958), the 400 KB per-file limit and
the total byte count. If the new file is over 400 KB, rerun step 4 with the other encoding
(`--lossless` on or off) and keep the smaller file.

## Step 8 — rebuild the app

Skins are embedded into the binary at build time by `src-tauri/build.rs`. An app that is
already built will not show the new skin. Tell the user this, and run the build if they want it
now:

```sh
pnpm tauri build      # or: pnpm tauri dev
```

## Step 9 — report

Tell the user, in a few lines:

- the id, name, collection, file, final `--focus` and file size;
- which skin was replaced, if any;
- that `skin check` passed;
- that the app needs a rebuild for the skin to appear.

Do not commit anything unless the user asks. `assets/previews/` is gitignored, so previews stay
out of the repository; the files to commit are `assets/skins/<id>.<ext>` and
`assets/skins/manifest.json`.

## Built-in packs

A themed set of pictures, such as renders from Grok Imagine, ships as a pack under
`assets/packs/<id>/` rather than as more manifest skins. docs/PACKS.md → Built-in packs has the
background.

1. Put the pictures in one folder. Skins are named after their files ("glass_folder.jpg" is
   "Glass folder"), so rename meaningless ones such as `grok-image-3.jpg` first; look at each
   with the Read tool to name it.
2. Make the pack. The first tag names the pack in the library's filters; the author is the
   user's GitHub name; the licence is `CC0-1.0` unless they say otherwise.

   ```sh
   cargo run -p folderskin-tools -- packs make <folder> --id <id> --name "<Name>" \
     --tags <id>,<more> --author <github-name> --preview /tmp/<id>.png
   ```

3. Read the report. Each picture is `folder` (cut out of magenta or transparency and used as
   the icon) or `artwork` (wrapped onto FolderSkin's folder). A render meant as a whole folder
   that came out as `artwork` wasn't on a clean enough magenta: say so, and ask for a re-render
   rather than forcing it. Without `cwebp` folders are PNG and often too big; install it
   (`brew install webp`) and run again.
4. Open the preview PNG with the Read tool. Look for magenta fringes, tabs cut off, a subject
   that doesn't fill the folder, and names that read badly.
5. Fix names in `assets/packs/<id>/pack.json` (1–60 characters) if needed, then run
   `cargo run -p folderskin-tools -- packs check --dir assets --max-kb 400`.
6. Rebuild. The skins appear in the library under the pack's first tag.

To start over, delete `assets/packs/<id>/`: `packs make` never overwrites a pack.

## Rules

- Only original art or CC0 art goes into `assets/skins/`. Fill in `author` and `license`
  honestly. Never commit art extracted from another product, a stock photo, a wallpaper, or
  model output whose licence you have not checked.
- 400 KB maximum per skin; keep the whole set under 2.5 MB.
- The shipped set is exactly ten skins — replace one rather than adding an eleventh — unless
  the maintainer says otherwise.
- Collections are `glow`, `grain` and `pop`. The gallery derives its tabs from the manifest,
  so a fourth collection appears on its own — keep to the three unless a maintainer asks for
  another, so the toolbar stays on one row.
- HEIC and HEIF inputs must be converted with `sips -s format png in.heic --out /tmp/in.png`
  first, and that only works on macOS.
- Always rebuild after changing a skin, because the binary embeds them.
- Never hand-edit `assets/skins/manifest.json`, and never delete a skin file without removing
  its manifest entry through the tool.

## Other commands in this tool

| command | use |
|---|---|
| `cargo run -p folderskin-tools -- skin gen` | regenerate all ten built-in skins, the manifest and the previews, deterministically. This overwrites imported skins, so only run it when the user asks to reset the set |
| `cargo run -p folderskin-tools -- render "<image>" --out /tmp/icon.png --size 512 --focus x,y` | render the finished icon from any picture without touching the manifest — the fastest way to test a focus point before importing |
| `cargo run -p folderskin-tools -- render --solid RRGGBB --out /tmp/flat.png` | render the template over a flat colour, to inspect the template itself |
| `cargo run -p folderskin-tools -- apply "<folder>" --skin <id>` | apply a skin to a real folder from the terminal |
| `cargo run -p folderskin-tools -- revert "<folder>"` | put the default icon back |
| `cargo run -p folderskin-tools -- packs make <pictures> --id … --name … --tags … --author …` | make a pack from pictures, built in (`--dir assets`, the default) or for the community (`--dir community`) |
| `cargo run -p folderskin-tools -- packs check --dir assets --max-kb 400` | check the built-in packs the way CI does |
